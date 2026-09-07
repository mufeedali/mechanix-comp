use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use animation::{Animated, AnimationConfig, Easing, monotonic_now};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Size};
use tracing::{info, warn};

use crate::chrome::{self, Fill, Handle};
use crate::config::{self, HomeConfig, IconConfig, SlotConfig};
use crate::pages::Pages;

const GAP: i32 = 16;
const LONG_PRESS: Duration = Duration::from_millis(500);
const SLOP: f64 = 20.0;
/// Visual pop while lifted. Hole math uses the un-nudged rest position.
const LIFT_NUDGE_Y: i32 = -36;
const PAGE_COMMIT: f64 = 0.18;
const PAGE_FLICK: f64 = 900.0;
const PAGE_SETTLE: Duration = Duration::from_millis(220);
const PAGE_EDGE: f64 = 48.0;
const PAGE_EDGE_HOLD: Duration = Duration::from_millis(400);
const EDGE_RESIST: f64 = 0.32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemId(u32);

enum Kind {
    Widget {
        command: String,
        surface: Option<WlSurface>,
        child: Option<Child>,
        launch_failed: bool,
    },
    Icon {
        desktop: String,
        exec: String,
        color: [f32; 4],
    },
}

struct Item {
    id: ItemId,
    cells: RectCells,
    kind: Kind,
}

impl Item {
    fn is_widget(&self) -> bool {
        matches!(self.kind, Kind::Widget { .. })
    }

    fn pending_claim(&self) -> bool {
        matches!(
            self.kind,
            Kind::Widget {
                child: Some(_),
                surface: None,
                ..
            }
        )
    }

    fn ready_to_spawn(&self) -> bool {
        match &self.kind {
            Kind::Widget {
                child: None,
                surface: None,
                launch_failed: false,
                command,
                ..
            } => !command.trim().is_empty(),
            _ => false,
        }
    }

    fn owns_surface(&self, surface: &WlSurface) -> bool {
        matches!(&self.kind, Kind::Widget { surface: Some(s), .. } if s == surface)
    }
}

enum Gesture {
    Idle,
    Pending {
        start: Point<f64, Logical>,
        t0: Instant,
        item: Option<ItemId>,
        /// `Some` if this contact is a finger (`wl_touch` id).
        touch_id: Option<i32>,
    },
    Swipe {
        start_x: f64,
        origin_page: u32,
        carry: f64,
        last_x: f64,
        last_t: Instant,
        vel: f64,
    },
    Lifted {
        item: ItemId,
        start: Point<f64, Logical>,
    },
    Resizing {
        item: ItemId,
        handle: Handle,
        origin: RectCells,
        /// `pointer - handle_anchor` at press. The grabbed pixel stays put.
        grab: Point<f64, Logical>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RectCells {
    page: u32,
    col: u32,
    row: u32,
    col_span: u32,
    row_span: u32,
}

pub enum HomeAction {
    None,
    Redraw,
    /// Move the lifted surface in Space only — no xdg configure.
    Relocate,
    Relayout,
    TapWidget {
        pos: Point<f64, Logical>,
        /// Nest `wl_touch` slot. `None` is a nest pointer click.
        touch_id: Option<i32>,
    },
    Launch(String),
    CloseSurface(WlSurface),
    /// Config changed: close leftover nest surfaces, then relayout.
    Reload { close: Vec<WlSurface> },
}

pub struct Home {
    path: PathBuf,
    columns: u32,
    rows: u32,
    pages: Pages,
    items: Vec<Item>,
    output: Size<i32, Logical>,
    edit: bool,
    selected: Option<ItemId>,
    gesture: Gesture,
    page_drag: f64,
    settle: Option<Animated<f32>>,
    last_pos: Point<f64, Logical>,
    drag_snapshot: Option<Vec<(ItemId, RectCells)>>,
    lift_loc: Point<i32, Logical>,
    /// Pixel frame during resize. Follows the handle; snaps to the grid on release.
    frame_preview: Option<Rectangle<i32, Logical>>,
    page_edge: Option<(i32, Instant)>,
    wait: Option<Duration>,
}

impl Home {
    pub fn load() -> std::io::Result<Self> {
        let path = config::config_path();
        let cfg = config::load_or_init(&path)?;
        let mut next_id = 1u32;
        let mut items = Vec::new();
        for slot in &cfg.slots {
            items.push(Item {
                id: ItemId(next_id),
                cells: RectCells {
                    page: slot.page,
                    col: slot.col,
                    row: slot.row,
                    col_span: slot.col_span.max(1),
                    row_span: slot.row_span.max(1),
                },
                kind: Kind::Widget {
                    command: slot.command.clone(),
                    surface: None,
                    child: None,
                    launch_failed: false,
                },
            });
            next_id += 1;
        }
        for (i, icon) in cfg.icons.iter().enumerate() {
            let exec = parse_desktop(&icon.desktop);
            let hue = (i as f32 * 0.37) % 1.0;
            items.push(Item {
                id: ItemId(next_id),
                cells: RectCells {
                    page: icon.page,
                    col: icon.col,
                    row: icon.row,
                    col_span: 1,
                    row_span: 1,
                },
                kind: Kind::Icon {
                    desktop: icon.desktop.clone(),
                    exec,
                    color: [0.25 + 0.4 * hue, 0.35, 0.65 - 0.2 * hue, 1.0],
                },
            });
            next_id += 1;
        }
        let mut home = Self {
            path,
            columns: cfg.columns.max(1),
            rows: cfg.rows.max(1),
            pages: Pages::default(),
            items,
            output: Size::from((0, 0)),
            edit: false,
            selected: None,
            gesture: Gesture::Idle,
            page_drag: 0.0,
            settle: None,
            last_pos: Point::from((0.0, 0.0)),
            drag_snapshot: None,
            lift_loc: Point::from((0, 0)),
            frame_preview: None,
            page_edge: None,
            wait: None,
        };
        home.compact_pages();
        Ok(home)
    }

    pub fn set_output_size(&mut self, w: i32, h: i32) {
        self.output = Size::from((w, h));
    }

    fn page(&self) -> u32 {
        self.pages.current()
    }

    fn set_page(&mut self, page: u32) {
        self.pages.jump(page);
    }

    fn occupied_pages(&self) -> Vec<u32> {
        self.items.iter().map(|i| i.cells.page).collect()
    }

    fn carrying_page(&self) -> Option<u32> {
        match self.gesture {
            Gesture::Lifted { item, .. } => Some(self.cells_of(item).page),
            _ => None,
        }
    }

    fn compact_pages(&mut self) {
        if self.carrying_page().is_some() {
            return;
        }
        let map = Pages::compact(self.occupied_pages());
        for item in &mut self.items {
            if let Some(&page) = map.get(&item.cells.page) {
                item.cells.page = page;
            }
        }
        if let Some(snap) = &mut self.drag_snapshot {
            for (_, cells) in snap {
                if let Some(&page) = map.get(&cells.page) {
                    cells.page = page;
                }
            }
        }
        let next = map
            .get(&self.page())
            .copied()
            .unwrap_or_else(|| self.page_count().saturating_sub(1));
        self.set_page(next);
    }

    pub fn pump_spawns(&mut self, nest: &OsStr) {
        // GTK (and some xdg clients) abort Wayland init if there is no wl_output
        // yet, then fall through to X11. Wait for the host layer configure.
        if self.output.w <= 0 || self.output.h <= 0 {
            return;
        }
        if self.items.iter().any(Item::pending_claim) {
            return;
        }
        let Some(item) = self.items.iter_mut().find(|i| i.ready_to_spawn()) else {
            return;
        };
        let Kind::Widget {
            command,
            child,
            launch_failed,
            ..
        } = &mut item.kind
        else {
            return;
        };
        match spawn_on_nest(command, nest) {
            Ok(proc) => {
                info!(cmd = %command, pid = proc.id(), "spawned widget");
                *child = Some(proc);
            }
            Err(err) => {
                *launch_failed = true;
                warn!(cmd = %command, %err, "widget spawn failed");
            }
        }
    }

    pub fn reap(&mut self) {
        for item in &mut self.items {
            let Kind::Widget {
                child,
                surface,
                launch_failed,
                ..
            } = &mut item.kind
            else {
                continue;
            };
            let Some(proc) = child.as_mut() else {
                continue;
            };
            let Some(status) = proc.try_wait().ok().flatten() else {
                continue;
            };
            *child = None;
            if surface.is_none() {
                *launch_failed = true;
                warn!(?status, "widget exited before creating a surface");
            }
        }
    }

    pub fn claim(&mut self, surface: &WlSurface) {
        if self.items.iter().any(|i| i.owns_surface(surface)) {
            return;
        }
        let Some(item) = self.items.iter_mut().find(|i| i.pending_claim()) else {
            return;
        };
        if let Kind::Widget {
            command, surface: slot, ..
        } = &mut item.kind
        {
            info!(cmd = %command, "claimed nest toplevel for slot");
            *slot = Some(surface.clone());
        }
    }

    pub fn release(&mut self, surface: &WlSurface) {
        for item in &mut self.items {
            if let Kind::Widget { surface: slot, .. } = &mut item.kind
                && slot.as_ref() == Some(surface)
            {
                *slot = None;
            }
        }
    }

    /// Nest placement. A lifted widget follows the pointer; everything else
    /// sits on its current grid cell (the hole, once reflow has run).
    pub fn placement(&self, surface: &WlSurface) -> Option<Rectangle<i32, Logical>> {
        let item = self.items.iter().find(|i| i.owns_surface(surface))?;
        if self.lifted() == Some(item.id) {
            return Some(self.lift_rect(item.id));
        }
        Some(self.item_rect(item.id))
    }

    pub fn visible_surfaces(&self) -> Vec<WlSurface> {
        self.items
            .iter()
            .filter(|i| self.page_nearby(i.cells.page))
            .filter_map(|i| match &i.kind {
                Kind::Widget { surface, .. } => surface.clone(),
                Kind::Icon { .. } => None,
            })
            .collect()
    }

    fn page_count(&self) -> u32 {
        self.pages
            .count(&self.occupied_pages(), self.carrying_page())
    }

    fn shift_x(&self) -> i32 {
        self.pages.offset(0, self.output.w) + self.page_drag.round() as i32
    }

    fn cell(&self) -> (i32, i32) {
        let cols = self.columns.max(1) as i32;
        let rows = self.rows.max(1) as i32;
        let cw = ((self.output.w - GAP * (cols + 1)) / cols).max(1);
        let ch = ((self.output.h - GAP * (rows + 1)) / rows).max(1);
        (cw, ch)
    }

    fn rect_of(&self, cells: RectCells) -> Rectangle<i32, Logical> {
        let (cw, ch) = self.cell();
        let x = GAP
            + cells.col as i32 * (cw + GAP)
            + cells.page as i32 * self.output.w
            + self.shift_x();
        let y = GAP + cells.row as i32 * (ch + GAP);
        let w = cells.col_span as i32 * cw + (cells.col_span as i32 - 1) * GAP;
        let h = cells.row_span as i32 * ch + (cells.row_span as i32 - 1) * GAP;
        Rectangle::new((x, y).into(), (w, h).into())
    }

    fn item(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    fn cells_of(&self, id: ItemId) -> RectCells {
        self.item(id).map(|i| i.cells).unwrap_or(RectCells {
            page: 0,
            col: 0,
            row: 0,
            col_span: 1,
            row_span: 1,
        })
    }

    fn item_rect(&self, id: ItemId) -> Rectangle<i32, Logical> {
        self.rect_of(self.cells_of(id))
    }

    fn lifted(&self) -> Option<ItemId> {
        match self.gesture {
            Gesture::Lifted { item, .. } => Some(item),
            _ => None,
        }
    }

    pub fn lifted_surface(&self) -> Option<WlSurface> {
        match &self.item(self.lifted()?)?.kind {
            Kind::Widget { surface, .. } => surface.clone(),
            Kind::Icon { .. } => None,
        }
    }

    fn lift_rect(&self, id: ItemId) -> Rectangle<i32, Logical> {
        let size = self.rect_of(self.snapshot_cells(id)).size;
        Rectangle::new(self.lift_loc, size)
    }

    fn hit_item(&self, pos: Point<f64, Logical>) -> Option<ItemId> {
        let p = (pos.x as i32, pos.y as i32);
        self.items
            .iter()
            .find(|i| self.item_rect(i.id).contains(p))
            .map(|i| i.id)
    }

    fn chrome_tile(&self, id: ItemId) -> Rectangle<i32, Logical> {
        if self.lifted() == Some(id) {
            self.lift_rect(id)
        } else if self.selected == Some(id) {
            self.frame_preview.unwrap_or_else(|| self.item_rect(id))
        } else {
            self.item_rect(id)
        }
    }

    fn hit_close(&self, pos: Point<f64, Logical>) -> Option<ItemId> {
        if !self.edit || self.lifted().is_some() {
            return None;
        }
        let id = self.selected?;
        chrome::hit_close(self.chrome_tile(id), pos).then_some(id)
    }

    fn hit_handle(&self, pos: Point<f64, Logical>) -> Option<(ItemId, Handle)> {
        if !self.edit || self.lifted().is_some() {
            return None;
        }
        let id = self.selected?;
        if !self.item(id).is_some_and(Item::is_widget) {
            return None;
        }
        chrome::hit_handle(self.chrome_tile(id), pos).map(|handle| (id, handle))
    }

    pub fn last_pos(&self) -> Point<f64, Logical> {
        self.last_pos
    }

    pub fn pointer_down(&mut self, pos: Point<f64, Logical>) -> HomeAction {
        self.contact_down(pos, None)
    }

    pub fn touch_down(&mut self, pos: Point<f64, Logical>, id: i32) -> HomeAction {
        self.contact_down(pos, Some(id))
    }

    fn contact_down(&mut self, pos: Point<f64, Logical>, touch_id: Option<i32>) -> HomeAction {
        self.last_pos = pos;
        self.settle = None;
        if let Some(id) = self.hit_close(pos) {
            return self.close_item(id);
        }
        if let Some((id, handle)) = self.hit_handle(pos) {
            self.begin_drag();
            let tile = self.chrome_tile(id);
            self.frame_preview = Some(tile);
            let anchor = chrome::handle_anchor(tile, handle);
            self.gesture = Gesture::Resizing {
                item: id,
                handle,
                origin: self.cells_of(id),
                grab: (pos.x - anchor.x, pos.y - anchor.y).into(),
            };
            return HomeAction::Redraw;
        }
        let item = self.hit_item(pos);
        // Already editing: pick up immediately, no second long-press.
        if self.edit && let Some(id) = item {
            return self.start_lift(id, pos);
        }
        self.gesture = Gesture::Pending {
            start: pos,
            t0: Instant::now(),
            item,
            touch_id,
        };
        if item.is_some() {
            self.wait = Some(LONG_PRESS);
        }
        HomeAction::None
    }

    pub fn pointer_move(&mut self, pos: Point<f64, Logical>) -> HomeAction {
        self.last_pos = pos;
        match self.gesture {
            Gesture::Pending { start, .. } => {
                let dx = pos.x - start.x;
                let dy = pos.y - start.y;
                if dx.abs() > SLOP && dx.abs() > dy.abs() && !self.edit {
                    let origin_page = self.page();
                    let carry = self.page_drag;
                    self.gesture = Gesture::Swipe {
                        start_x: start.x,
                        origin_page,
                        carry,
                        last_x: pos.x,
                        last_t: Instant::now(),
                        vel: 0.0,
                    };
                    self.page_drag = self.swipe_delta(dx + carry, origin_page);
                    return HomeAction::Relocate;
                }
                HomeAction::None
            }
            Gesture::Swipe {
                start_x,
                origin_page,
                carry,
                last_x,
                last_t,
                vel,
            } => {
                let now = Instant::now();
                let dt = now.saturating_duration_since(last_t).as_secs_f64();
                let vel = if dt > 1e-4 {
                    vel * 0.65 + ((pos.x - last_x) / dt) * 0.35
                } else {
                    vel
                };
                self.page_drag = self.swipe_delta(pos.x - start_x + carry, origin_page);
                self.gesture = Gesture::Swipe {
                    start_x,
                    origin_page,
                    carry,
                    last_x: pos.x,
                    last_t: now,
                    vel,
                };
                HomeAction::Relocate
            }
            Gesture::Lifted { item, start } => self.drag_lift(item, start, pos),
            Gesture::Resizing {
                item,
                handle,
                origin,
                grab,
            } => self.resize_item(item, handle, origin, grab, pos),
            Gesture::Idle => HomeAction::None,
        }
    }

    pub fn pointer_up(&mut self, pos: Point<f64, Logical>) -> HomeAction {
        let g = std::mem::replace(&mut self.gesture, Gesture::Idle);
        match g {
            Gesture::Pending {
                start,
                t0,
                item,
                touch_id,
            } => {
                let dt = Instant::now().saturating_duration_since(t0);
                let dist = (pos.x - start.x).hypot(pos.y - start.y);
                if dt >= LONG_PRESS && dist <= SLOP && let Some(id) = item {
                    self.enter_edit(id);
                    return HomeAction::Redraw;
                }
                if dist <= SLOP {
                    if self.edit {
                        if item.is_none() {
                            self.edit = false;
                            self.selected = None;
                            return HomeAction::Redraw;
                        }
                        if let Some(id) = item {
                            self.selected = Some(id);
                            return HomeAction::Redraw;
                        }
                    }
                    return match item.and_then(|id| self.item(id)).map(|i| &i.kind) {
                        Some(Kind::Widget { .. }) => HomeAction::TapWidget { pos, touch_id },
                        Some(Kind::Icon { exec, .. }) => HomeAction::Launch(exec.clone()),
                        None => HomeAction::None,
                    };
                }
                HomeAction::None
            }
            Gesture::Swipe {
                start_x,
                origin_page,
                carry,
                vel,
                ..
            } => {
                let dx = self.swipe_delta(pos.x - start_x + carry, origin_page);
                let width = self.output.w.max(1) as f64;
                let mut page = origin_page as i32;
                if dx < -width * PAGE_COMMIT || vel < -PAGE_FLICK {
                    page += 1;
                } else if dx > width * PAGE_COMMIT || vel > PAGE_FLICK {
                    page -= 1;
                }
                let last = self.page_count().saturating_sub(1) as i32;
                self.set_page(page.clamp(0, last) as u32);
                let visual = -(origin_page as f64) * width + dx;
                self.page_drag = visual + self.page() as f64 * width;
                self.settle = Some(Animated::new(
                    self.page_drag as f32,
                    0.0,
                    AnimationConfig::new(PAGE_SETTLE, Easing::EaseOut),
                    monotonic_now(),
                ));
                HomeAction::Relocate
            }
            Gesture::Lifted { .. } | Gesture::Resizing { .. } => {
                if self.outside_grid(pos) {
                    self.restore_snapshot();
                }
                self.frame_preview = None;
                self.drag_snapshot = None;
                self.page_edge = None;
                self.pages.end_lift();
                self.compact_pages();
                self.persist();
                HomeAction::Relayout
            }
            Gesture::Idle => HomeAction::None,
        }
    }

    pub fn tick(&mut self) -> HomeAction {
        let lift = self.tick_long_press();
        if !matches!(lift, HomeAction::None) {
            return lift;
        }
        if let Gesture::Lifted { item, start } = self.gesture
            && self.page_edge.is_some()
        {
            return self.drag_lift(item, start, self.last_pos);
        }
        self.tick_settle()
    }

    pub fn take_wait(&mut self) -> Option<Duration> {
        self.wait.take()
    }

    fn tick_long_press(&mut self) -> HomeAction {
        let Gesture::Pending {
            start,
            t0,
            item,
            ..
        } = self.gesture
        else {
            return HomeAction::None;
        };
        let Some(id) = item else {
            return HomeAction::None;
        };
        if Instant::now().saturating_duration_since(t0) < LONG_PRESS {
            return HomeAction::None;
        }
        self.start_lift(id, start)
    }

    fn tick_settle(&mut self) -> HomeAction {
        let Some(anim) = &self.settle else {
            return HomeAction::None;
        };
        if !matches!(self.gesture, Gesture::Idle) {
            self.settle = None;
            return HomeAction::None;
        }
        let now = monotonic_now();
        if anim.is_finished(now) {
            self.page_drag = 0.0;
            self.settle = None;
            return HomeAction::Relocate;
        }
        self.page_drag = anim.get(now) as f64;
        HomeAction::Relocate
    }

    fn swipe_delta(&self, raw: f64, origin: u32) -> f64 {
        let at_first = origin == 0 && raw > 0.0;
        let at_last = origin + 1 >= self.page_count() && raw < 0.0;
        if at_first || at_last {
            raw * EDGE_RESIST
        } else {
            raw
        }
    }

    fn enter_edit(&mut self, id: ItemId) {
        self.edit = true;
        self.selected = Some(id);
        info!("edit mode");
    }

    fn start_lift(&mut self, id: ItemId, start: Point<f64, Logical>) -> HomeAction {
        self.enter_edit(id);
        self.pages.begin_lift(self.page_count());
        self.begin_drag();
        self.page_edge = None;
        self.gesture = Gesture::Lifted { item: id, start };
        info!("lifted item");
        self.drag_lift(id, start, start)
    }

    fn begin_drag(&mut self) {
        self.drag_snapshot = Some(
            self.items
                .iter()
                .map(|i| (i.id, i.cells))
                .collect(),
        );
    }

    fn restore_snapshot(&mut self) {
        let Some(snapshot) = self.drag_snapshot.take() else {
            return;
        };
        for &(id, cells) in &snapshot {
            self.apply_cells(id, cells);
        }
        self.drag_snapshot = Some(snapshot);
    }

    fn outside_grid(&self, pos: Point<f64, Logical>) -> bool {
        pos.x < 0.0
            || pos.y < 0.0
            || pos.x >= self.output.w as f64
            || pos.y >= self.output.h as f64
    }

    fn snapshot_cells(&self, id: ItemId) -> RectCells {
        self.drag_snapshot
            .as_ref()
            .and_then(|s| s.iter().find(|(i, _)| *i == id).map(|(_, c)| *c))
            .unwrap_or_else(|| self.cells_of(id))
    }

    /// Lift follows the pointer. Occupancy uses a hole snapped under the
    /// un-nudged rest position; neighbors reflow from the press-down snapshot.
    fn drag_lift(
        &mut self,
        id: ItemId,
        start: Point<f64, Logical>,
        pos: Point<f64, Logical>,
    ) -> HomeAction {
        self.follow_lift(id, start, pos);
        if let Some(page) = self.edge_page(pos) {
            return self.lift_to_page(id, pos, page);
        }
        if self.outside_grid(pos) {
            self.page_edge = None;
            self.restore_snapshot();
            return HomeAction::Relayout;
        }
        self.place_lift(id, start, pos)
    }

    fn follow_lift(&mut self, id: ItemId, start: Point<f64, Logical>, pos: Point<f64, Logical>) {
        let rest = self.lift_rest(id, start, pos);
        self.lift_loc = (rest.x, rest.y + LIFT_NUDGE_Y).into();
    }

    fn lift_rest(&self, id: ItemId, start: Point<f64, Logical>, pos: Point<f64, Logical>) -> Point<i32, Logical> {
        let origin_rect = self.rect_of(self.snapshot_cells(id));
        Point::from((
            origin_rect.loc.x + (pos.x - start.x).round() as i32,
            origin_rect.loc.y + (pos.y - start.y).round() as i32,
        ))
    }

    fn place_lift(
        &mut self,
        id: ItemId,
        start: Point<f64, Logical>,
        pos: Point<f64, Logical>,
    ) -> HomeAction {
        let origin = self.snapshot_cells(id);
        let hole = self.hole_at(self.lift_rest(id, start, pos), origin);
        if self.cells_of(id) == hole {
            return HomeAction::Relocate;
        }
        if self.try_place(id, hole, push_dirs(origin, hole, None)) {
            HomeAction::Relayout
        } else {
            self.restore_snapshot();
            HomeAction::Relayout
        }
    }

    fn edge_dir(&self, pos: Point<f64, Logical>) -> Option<i32> {
        let width = self.output.w as f64;
        if width <= 0.0 {
            return None;
        }
        if pos.x <= PAGE_EDGE && self.page() > 0 {
            Some(-1)
        } else if pos.x >= width - PAGE_EDGE && self.page() < self.page_count() {
            Some(1)
        } else {
            None
        }
    }

    fn edge_page(&mut self, pos: Point<f64, Logical>) -> Option<u32> {
        let Some(dir) = self.edge_dir(pos) else {
            self.page_edge = None;
            return None;
        };
        let now = Instant::now();
        match self.page_edge {
            Some((prev, t0)) if prev == dir && now.saturating_duration_since(t0) >= PAGE_EDGE_HOLD => {
                let next = self.page() as i32 + dir;
                let max = self.page_count() as i32;
                if next < 0 || next > max {
                    return None;
                }
                self.page_edge = Some((dir, now));
                self.wait = Some(PAGE_EDGE_HOLD);
                Some(next as u32)
            }
            Some((prev, _)) if prev == dir => None,
            _ => {
                self.page_edge = Some((dir, now));
                self.wait = Some(PAGE_EDGE_HOLD);
                None
            }
        }
    }

    fn lift_to_page(&mut self, id: ItemId, pos: Point<f64, Logical>, page: u32) -> HomeAction {
        let visual = self.lift_loc;
        self.restore_snapshot();
        self.set_page(page);
        self.begin_drag();
        let origin = self.rect_of(self.snapshot_cells(id));
        let rest: Point<i32, Logical> = Point::from((visual.x, visual.y - LIFT_NUDGE_Y));
        let start = Point::from((
            pos.x - (rest.x - origin.loc.x) as f64,
            pos.y - (rest.y - origin.loc.y) as f64,
        ));
        self.gesture = Gesture::Lifted { item: id, start };
        info!(page, "lifted item to page");
        self.place_lift(id, start, pos)
    }

    fn hole_at(&self, lift_loc: Point<i32, Logical>, origin: RectCells) -> RectCells {
        let (cw, ch) = self.cell();
        let pitch_x = (cw + GAP) as f64;
        let pitch_y = (ch + GAP) as f64;
        let col = nearest_cell(
            lift_loc.x as f64,
            GAP as f64,
            pitch_x,
            self.columns.saturating_sub(origin.col_span),
        );
        let row = nearest_cell(
            lift_loc.y as f64,
            GAP as f64,
            pitch_y,
            self.rows.saturating_sub(origin.row_span),
        );
        RectCells {
            page: self.page(),
            col,
            row,
            col_span: origin.col_span,
            row_span: origin.row_span,
        }
    }

    fn resize_item(
        &mut self,
        id: ItemId,
        handle: Handle,
        origin: RectCells,
        grab: Point<f64, Logical>,
        pos: Point<f64, Logical>,
    ) -> HomeAction {
        let origin_rect = self.rect_of(origin);
        let (cw, ch) = self.cell();
        let at = Point::from((pos.x - grab.x, pos.y - grab.y));
        self.frame_preview = Some(resize_preview(origin_rect, handle, at, cw, ch));
        if self.outside_grid(pos) {
            self.restore_snapshot();
            return HomeAction::Relayout;
        }
        let next = self.resize_cells(origin, origin_rect, handle, at);
        if self.cells_of(id) == next {
            return HomeAction::Redraw;
        }
        if self.try_place(id, next, push_dirs(origin, next, Some(handle))) {
            HomeAction::Relayout
        } else {
            self.restore_snapshot();
            HomeAction::Relayout
        }
    }

    /// Reserve `next` for `id`. Everyone else is pushed in one direction as a
    /// cascade so neighbors stay in order instead of teleporting to a hole.
    fn try_place(&mut self, id: ItemId, next: RectCells, dirs: [Dir; 4]) -> bool {
        if next.col + next.col_span > self.columns || next.row + next.row_span > self.rows {
            return false;
        }
        let owned;
        let snapshot: &[(ItemId, RectCells)] = match &self.drag_snapshot {
            Some(s) => s,
            None => {
                owned = self.items.iter().map(|i| (i.id, i.cells)).collect::<Vec<_>>();
                &owned
            }
        };
        for dir in dirs {
            if let Some(placed) =
                push_resolve(snapshot, id, next, dir, self.columns, self.rows)
            {
                for (item, cells) in placed {
                    self.apply_cells(item, cells);
                }
                return true;
            }
        }
        false
    }

    fn apply_cells(&mut self, id: ItemId, cells: RectCells) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.cells = cells;
        }
    }

    fn close_item(&mut self, id: ItemId) -> HomeAction {
        self.gesture = Gesture::Idle;
        self.drag_snapshot = None;
        self.frame_preview = None;
        self.selected = None;
        let Some(idx) = self.items.iter().position(|i| i.id == id) else {
            return HomeAction::None;
        };
        let item = self.items.remove(idx);
        let action = match item.kind {
            Kind::Widget {
                mut child, surface, ..
            } => {
                if let Some(mut proc) = child.take() {
                    let _ = proc.kill();
                }
                match surface {
                    Some(s) => HomeAction::CloseSurface(s),
                    None => HomeAction::Relayout,
                }
            }
            Kind::Icon { .. } => HomeAction::Redraw,
        };
        self.compact_pages();
        self.persist();
        action
    }

    pub fn persist(&self) {
        if let Err(err) = config::save(&self.path, &self.current_config()) {
            warn!(%err, "failed to write homescreen config");
        }
    }

    pub fn reload(&mut self) -> HomeAction {
        let cfg = match config::load(&self.path) {
            Ok(cfg) => cfg,
            Err(err) => {
                warn!(%err, "homescreen config reload failed");
                return HomeAction::None;
            }
        };
        if self.current_config() == cfg {
            return HomeAction::None;
        }
        info!("reloaded homescreen config");
        self.apply_config(cfg)
    }

    fn current_config(&self) -> HomeConfig {
        let mut slots = Vec::new();
        let mut icons = Vec::new();
        for item in &self.items {
            match &item.kind {
                Kind::Widget { command, .. } => slots.push(SlotConfig {
                    page: item.cells.page,
                    col: item.cells.col,
                    row: item.cells.row,
                    col_span: item.cells.col_span,
                    row_span: item.cells.row_span,
                    command: command.clone(),
                }),
                Kind::Icon { desktop, .. } => icons.push(IconConfig {
                    page: item.cells.page,
                    col: item.cells.col,
                    row: item.cells.row,
                    desktop: desktop.clone(),
                }),
            }
        }
        HomeConfig {
            columns: self.columns,
            rows: self.rows,
            slots,
            icons,
        }
    }

    fn apply_config(&mut self, cfg: HomeConfig) -> HomeAction {
        let old = std::mem::take(&mut self.items);
        let mut old_widgets = Vec::new();
        let mut old_icons = Vec::new();
        for item in old {
            if item.is_widget() {
                old_widgets.push(item);
            } else {
                old_icons.push(item);
            }
        }
        let mut next_id = old_widgets
            .iter()
            .chain(&old_icons)
            .map(|i| i.id.0)
            .max()
            .unwrap_or(0)
            + 1;

        let mut items = Vec::new();
        for slot in &cfg.slots {
            let cells = cells_of_slot(slot);
            if let Some(idx) = take_widget(&old_widgets, slot) {
                let mut item = old_widgets.remove(idx);
                item.cells = cells;
                items.push(item);
            } else {
                items.push(Item {
                    id: ItemId(next_id),
                    cells,
                    kind: Kind::Widget {
                        command: slot.command.clone(),
                        surface: None,
                        child: None,
                        launch_failed: false,
                    },
                });
                next_id += 1;
            }
        }

        let mut close = Vec::new();
        for item in old_widgets {
            if let Kind::Widget {
                mut child, surface, ..
            } = item.kind
            {
                if let Some(mut proc) = child.take() {
                    let _ = proc.kill();
                }
                if let Some(surface) = surface {
                    close.push(surface);
                }
            }
        }

        for (i, icon) in cfg.icons.iter().enumerate() {
            let cells = cells_of_icon(icon);
            if let Some(idx) = take_icon(&old_icons, icon) {
                let mut item = old_icons.remove(idx);
                item.cells = cells;
                items.push(item);
            } else {
                let hue = (i as f32 * 0.37) % 1.0;
                items.push(Item {
                    id: ItemId(next_id),
                    cells,
                    kind: Kind::Icon {
                        desktop: icon.desktop.clone(),
                        exec: parse_desktop(&icon.desktop),
                        color: [0.25 + 0.4 * hue, 0.35, 0.65 - 0.2 * hue, 1.0],
                    },
                });
                next_id += 1;
            }
        }

        self.columns = cfg.columns.max(1);
        self.rows = cfg.rows.max(1);
        self.items = items;
        self.compact_pages();
        if self.current_config() != cfg {
            self.persist();
        }
        if self.selected.is_some_and(|id| self.item(id).is_none()) {
            self.selected = None;
        }
        let gone = match self.gesture {
            Gesture::Lifted { item, .. } | Gesture::Resizing { item, .. } => {
                self.item(item).is_none()
            }
            Gesture::Pending { item: Some(id), .. } => self.item(id).is_none(),
            _ => false,
        };
        if gone {
            self.gesture = Gesture::Idle;
            self.frame_preview = None;
            self.drag_snapshot = None;
            self.page_edge = None;
            self.pages.end_lift();
        }

        HomeAction::Reload { close }
    }

    pub fn launch(&self, exec: &str) {
        if exec.is_empty() {
            return;
        }
        let Some(display) = std::env::var_os("WAYLAND_DISPLAY") else {
            warn!(exec, "icon launch skipped: no host WAYLAND_DISPLAY");
            return;
        };
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(exec);
        apply_wayland_only(&mut cmd, &display);
        match cmd.spawn() {
            Ok(child) => info!(pid = child.id(), exec, "launched icon on host"),
            Err(err) => warn!(%err, exec, "icon launch failed"),
        }
    }

    pub fn fills(&self) -> Vec<Fill> {
        let mut out = Vec::new();
        if self.edit || self.lifted().is_some() {
            out.extend(self.grid_dots());
        }
        for item in &self.items {
            let Kind::Icon { color, .. } = &item.kind else {
                continue;
            };
            if !self.page_nearby(item.cells.page) && self.lifted() != Some(item.id) {
                continue;
            }
            out.push(chrome::fill(self.chrome_tile(item.id), *color));
        }
        if self.edit {
            for item in &self.items {
                if Some(item.id) == self.lifted() || Some(item.id) == self.selected {
                    continue;
                }
                if !self.page_nearby(item.cells.page) {
                    continue;
                }
                out.extend(chrome::idle_border(self.item_rect(item.id)));
            }
        }
        if let Some(id) = self.lifted() {
            out.extend(chrome::hole_overlay(self.item_rect(id)));
            out.extend(chrome::lift_overlay(self.lift_rect(id)));
        } else if self.edit && let Some(id) = self.selected {
            out.extend(chrome::edit_overlay(
                self.chrome_tile(id),
                self.item(id).is_some_and(Item::is_widget),
            ));
        }
        out
    }

    /// Dots at every gap intersection on the current page (cols+1 × rows+1).
    fn grid_dots(&self) -> Vec<Fill> {
        let (cw, ch) = self.cell();
        let cols = self.columns.max(1);
        let rows = self.rows.max(1);
        let origin_x = self.page() as i32 * self.output.w + self.shift_x();
        let mut out = Vec::with_capacity(((cols + 1) * (rows + 1)) as usize);
        for r in 0..=rows {
            for c in 0..=cols {
                let x = GAP / 2 + c as i32 * (cw + GAP) + origin_x;
                let y = GAP / 2 + r as i32 * (ch + GAP);
                out.push(chrome::grid_dot((x, y).into()));
            }
        }
        out
    }

    fn page_nearby(&self, page: u32) -> bool {
        self.pages.nearby(page)
    }

    fn resize_cells(
        &self,
        origin: RectCells,
        origin_rect: Rectangle<i32, Logical>,
        handle: Handle,
        pos: Point<f64, Logical>,
    ) -> RectCells {
        let (cw, ch) = self.cell();
        let mut next = origin;
        let pitch_x = (cw + GAP) as f64;
        let pitch_y = (ch + GAP) as f64;
        if handle.east() {
            next.col_span = snap_span(
                pos.x - origin_rect.loc.x as f64,
                cw as f64,
                pitch_x,
                self.columns.saturating_sub(origin.col),
            );
        }
        if handle.south() {
            next.row_span = snap_span(
                pos.y - origin_rect.loc.y as f64,
                ch as f64,
                pitch_y,
                self.rows.saturating_sub(origin.row),
            );
        }
        if handle.west() {
            let right = origin.col + origin.col_span;
            let span = snap_span(
                (origin_rect.loc.x + origin_rect.size.w) as f64 - pos.x,
                cw as f64,
                pitch_x,
                right.min(self.columns),
            );
            next.col = right.saturating_sub(span);
            next.col_span = span;
        }
        if handle.north() {
            let bottom = origin.row + origin.row_span;
            let span = snap_span(
                (origin_rect.loc.y + origin_rect.size.h) as f64 - pos.y,
                ch as f64,
                pitch_y,
                bottom.min(self.rows),
            );
            next.row = bottom.saturating_sub(span);
            next.row_span = span;
        }
        next.col_span = next.col_span.min(self.columns.saturating_sub(next.col)).max(1);
        next.row_span = next.row_span.min(self.rows.saturating_sub(next.row)).max(1);
        next
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    fn step(self, cells: &mut RectCells, columns: u32, rows: u32) -> bool {
        match self {
            Dir::Left => {
                if cells.col == 0 {
                    return false;
                }
                cells.col -= 1;
                true
            }
            Dir::Right => {
                if cells.col + cells.col_span >= columns {
                    return false;
                }
                cells.col += 1;
                true
            }
            Dir::Up => {
                if cells.row == 0 {
                    return false;
                }
                cells.row -= 1;
                true
            }
            Dir::Down => {
                if cells.row + cells.row_span >= rows {
                    return false;
                }
                cells.row += 1;
                true
            }
        }
    }

    fn coord(self, cells: RectCells) -> i32 {
        match self {
            Dir::Left => -(cells.col as i32),
            Dir::Right => (cells.col + cells.col_span) as i32,
            Dir::Up => -(cells.row as i32),
            Dir::Down => (cells.row + cells.row_span) as i32,
        }
    }
}

fn push_dirs(from: RectCells, to: RectCells, handle: Option<Handle>) -> [Dir; 4] {
    let mut ordered = Vec::new();
    if let Some(handle) = handle {
        if handle.south() {
            ordered.push(Dir::Down);
        }
        if handle.east() {
            ordered.push(Dir::Right);
        }
        if handle.north() {
            ordered.push(Dir::Up);
        }
        if handle.west() {
            ordered.push(Dir::Left);
        }
    }
    let dc = to.col as i32 - from.col as i32;
    let dr = to.row as i32 - from.row as i32;
    if dr.abs() >= dc.abs() {
        ordered.push(if dr >= 0 { Dir::Down } else { Dir::Up });
        ordered.push(if dc >= 0 { Dir::Right } else { Dir::Left });
    } else {
        ordered.push(if dc >= 0 { Dir::Right } else { Dir::Left });
        ordered.push(if dr >= 0 { Dir::Down } else { Dir::Up });
    }
    for dir in [Dir::Down, Dir::Right, Dir::Left, Dir::Up] {
        if !ordered.contains(&dir) {
            ordered.push(dir);
        }
    }
    [ordered[0], ordered[1], ordered[2], ordered[3]]
}

fn snapshot_of(snapshot: &[(ItemId, RectCells)], id: ItemId) -> Option<RectCells> {
    snapshot.iter().find(|(i, _)| *i == id).map(|(_, c)| *c)
}

/// Push overlapping items in `dir` until the hole is free and no two items
/// overlap. An item in the way of a pushed neighbor is pushed too, so a
/// widget and the icon below it move as a group instead of one teleporting.
fn push_resolve(
    snapshot: &[(ItemId, RectCells)],
    mover: ItemId,
    hole: RectCells,
    dir: Dir,
    columns: u32,
    rows: u32,
) -> Option<Vec<(ItemId, RectCells)>> {
    let mut pos: Vec<(ItemId, RectCells)> = snapshot
        .iter()
        .map(|(id, cells)| {
            if *id == mover {
                (*id, hole)
            } else {
                (*id, *cells)
            }
        })
        .collect();
    let limit = (columns * rows) as usize * pos.len().max(1);
    for _ in 0..limit {
        let Some(i) = first_conflict(&pos, mover, dir, snapshot, hole.page) else {
            return Some(pos);
        };
        if !dir.step(&mut pos[i].1, columns, rows) {
            return None;
        }
    }
    None
}

fn first_conflict(
    pos: &[(ItemId, RectCells)],
    mover: ItemId,
    dir: Dir,
    snapshot: &[(ItemId, RectCells)],
    page: u32,
) -> Option<usize> {
    let mover_cells = pos.iter().find(|(id, _)| *id == mover)?.1;
    for (i, (id, cells)) in pos.iter().enumerate() {
        if *id == mover || cells.page != page {
            continue;
        }
        if overlaps(*cells, mover_cells) {
            return Some(i);
        }
    }
    for i in 0..pos.len() {
        if pos[i].0 == mover || pos[i].1.page != page {
            continue;
        }
        for j in (i + 1)..pos.len() {
            if pos[j].0 == mover || pos[j].1.page != page {
                continue;
            }
            if !overlaps(pos[i].1, pos[j].1) {
                continue;
            }
            let a = snapshot_of(snapshot, pos[i].0).unwrap_or(pos[i].1);
            let b = snapshot_of(snapshot, pos[j].0).unwrap_or(pos[j].1);
            return Some(if dir.coord(a) >= dir.coord(b) { i } else { j });
        }
    }
    None
}

fn resize_preview(
    origin: Rectangle<i32, Logical>,
    handle: Handle,
    pos: Point<f64, Logical>,
    min_w: i32,
    min_h: i32,
) -> Rectangle<i32, Logical> {
    let mut preview = origin;
    if handle.east() {
        preview.size.w = (pos.x as i32 - preview.loc.x).max(min_w);
    }
    if handle.south() {
        preview.size.h = (pos.y as i32 - preview.loc.y).max(min_h);
    }
    if handle.west() {
        let right = origin.loc.x + origin.size.w;
        preview.size.w = (right - pos.x as i32).max(min_w);
        preview.loc.x = right - preview.size.w;
    }
    if handle.north() {
        let bottom = origin.loc.y + origin.size.h;
        preview.size.h = (bottom - pos.y as i32).max(min_h);
        preview.loc.y = bottom - preview.size.h;
    }
    preview
}

/// Snap a top-left to the nearest cell. Threshold is halfway to the next cell.
fn nearest_cell(pos: f64, origin: f64, pitch: f64, max: u32) -> u32 {
    if pitch <= 0.0 {
        return 0;
    }
    let idx = ((pos - origin) / pitch).round() as i32;
    idx.clamp(0, max as i32) as u32
}

/// How many cells a dragged edge covers. Grows/shrinks past the midpoint of a cell.
fn snap_span(length: f64, cell: f64, pitch: f64, max: u32) -> u32 {
    if pitch <= 0.0 || max == 0 {
        return 1;
    }
    let span = ((length - cell / 2.0) / pitch).floor() as i32 + 1;
    span.clamp(1, max as i32) as u32
}

fn cells_of_slot(slot: &SlotConfig) -> RectCells {
    RectCells {
        page: slot.page,
        col: slot.col,
        row: slot.row,
        col_span: slot.col_span.max(1),
        row_span: slot.row_span.max(1),
    }
}

fn cells_of_icon(icon: &IconConfig) -> RectCells {
    RectCells {
        page: icon.page,
        col: icon.col,
        row: icon.row,
        col_span: 1,
        row_span: 1,
    }
}

fn take_widget(old: &[Item], slot: &SlotConfig) -> Option<usize> {
    let cells = cells_of_slot(slot);
    old.iter()
        .position(|item| {
            matches!(&item.kind, Kind::Widget { command, .. } if command == &slot.command)
                && item.cells == cells
        })
        .or_else(|| {
            old.iter().position(|item| {
                matches!(&item.kind, Kind::Widget { command, .. } if command == &slot.command)
            })
        })
}

fn take_icon(old: &[Item], icon: &IconConfig) -> Option<usize> {
    let cells = cells_of_icon(icon);
    old.iter()
        .position(|item| {
            matches!(&item.kind, Kind::Icon { desktop, .. } if desktop == &icon.desktop)
                && item.cells == cells
        })
        .or_else(|| {
            old.iter().position(|item| {
                matches!(&item.kind, Kind::Icon { desktop, .. } if desktop == &icon.desktop)
            })
        })
}

fn overlaps(a: RectCells, b: RectCells) -> bool {
    a.page == b.page
        && a.col < b.col + b.col_span
        && b.col < a.col + a.col_span
        && a.row < b.row + b.row_span
        && b.row < a.row + a.row_span
}

fn spawn_on_nest(command: &str, nest: &OsStr) -> std::io::Result<Child> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    apply_wayland_only(&mut cmd, nest);
    cmd.spawn()
}

/// Force Wayland and drop X11 so a host-launched GTK app cannot fall through
/// to GNOME's Xwayland when mechanix-comp is itself nested.
fn apply_wayland_only(cmd: &mut Command, display: &OsStr) {
    cmd.env("WAYLAND_DISPLAY", display)
        .env("GDK_BACKEND", "wayland")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_SOCKET");
}

fn parse_desktop(path: &str) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let mut exec = String::new();
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line.eq_ignore_ascii_case("[Desktop Entry]");
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Exec=") {
            exec = strip_field_codes(rest);
        }
    }
    exec
}

fn strip_field_codes(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|t| !t.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget(id: u32, page: u32, col: u32, row: u32) -> Item {
        Item {
            id: ItemId(id),
            cells: RectCells {
                page,
                col,
                row,
                col_span: 1,
                row_span: 1,
            },
            kind: Kind::Widget {
                command: "true".into(),
                surface: None,
                child: None,
                launch_failed: false,
            },
        }
    }

    fn test_home(items: Vec<Item>) -> Home {
        Home {
            path: PathBuf::from("/tmp/homescreen-test.toml"),
            columns: 4,
            rows: 6,
            pages: Pages::default(),
            items,
            output: Size::from((1280, 800)),
            edit: false,
            selected: None,
            gesture: Gesture::Idle,
            page_drag: 0.0,
            settle: None,
            last_pos: Point::from((0.0, 0.0)),
            drag_snapshot: None,
            lift_loc: Point::from((0, 0)),
            frame_preview: None,
            page_edge: None,
            wait: None,
        }
    }

    #[test]
    fn edge_hold_moves_lifted_item_to_the_next_page() {
        let mut home = test_home(vec![widget(1, 0, 0, 0), widget(2, 1, 0, 0)]);
        let id = ItemId(1);
        home.start_lift(id, (40.0, 40.0).into());
        assert_eq!(home.page(), 0);
        home.pointer_move((1270.0, 40.0).into());
        assert_eq!(home.page(), 0);
        assert!(home.page_edge.is_some());
        if let Some((_, t0)) = home.page_edge.as_mut() {
            *t0 = Instant::now() - PAGE_EDGE_HOLD - Duration::from_millis(1);
        }
        home.tick();
        assert_eq!(home.page(), 1);
        assert_eq!(home.cells_of(id).page, 1);
    }

    #[test]
    fn edge_hold_moves_lifted_item_to_the_previous_page() {
        let mut home = test_home(vec![widget(1, 1, 0, 0), widget(2, 0, 0, 0)]);
        home.set_page(1);
        let id = ItemId(1);
        home.start_lift(id, (640.0, 40.0).into());
        home.pointer_move((10.0, 40.0).into());
        if let Some((_, t0)) = home.page_edge.as_mut() {
            *t0 = Instant::now() - PAGE_EDGE_HOLD - Duration::from_millis(1);
        }
        home.tick();
        assert_eq!(home.page(), 0);
        assert_eq!(home.cells_of(id).page, 0);
    }

    #[test]
    fn compact_pages_removes_a_middle_hole() {
        let mut home = test_home(vec![widget(1, 0, 0, 0), widget(2, 2, 0, 0)]);
        home.compact_pages();
        assert_eq!(home.cells_of(ItemId(1)).page, 0);
        assert_eq!(home.cells_of(ItemId(2)).page, 1);
        assert_eq!(home.page_count(), 2);
    }

    #[test]
    fn reload_moves_a_widget_slot_without_respawning() {
        let path = std::env::temp_dir().join("homescreen-reload-test.toml");
        let mut home = test_home(vec![widget(1, 0, 0, 0)]);
        home.path = path.clone();
        let toml = r#"
columns = 4
rows = 6
[[slots]]
page = 0
col = 2
row = 1
col_span = 1
row_span = 1
command = "true"
"#;
        std::fs::write(&path, toml).unwrap();
        let action = home.reload();
        assert!(matches!(action, HomeAction::Reload { close } if close.is_empty()));
        assert_eq!(home.cells_of(ItemId(1)).col, 2);
        assert_eq!(home.cells_of(ItemId(1)).row, 1);
        let _ = std::fs::remove_file(path);
    }
}
