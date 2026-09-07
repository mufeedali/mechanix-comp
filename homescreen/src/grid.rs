//! Cell geometry and reflow. No Wayland.

use smithay::utils::{Logical, Point, Rectangle};

use crate::chrome::Handle;

pub(crate) const GAP: i32 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RectCells {
    pub page: u32,
    pub col: u32,
    pub row: u32,
    pub col_span: u32,
    pub row_span: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dir {
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

pub(crate) fn push_dirs(from: RectCells, to: RectCells, handle: Option<Handle>) -> [Dir; 4] {
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

pub(crate) fn push_resolve<Id: Copy + Eq>(
    snapshot: &[(Id, RectCells)],
    mover: Id,
    hole: RectCells,
    dir: Dir,
    columns: u32,
    rows: u32,
) -> Option<Vec<(Id, RectCells)>> {
    let mut pos: Vec<(Id, RectCells)> = snapshot
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

fn first_conflict<Id: Copy + Eq>(
    pos: &[(Id, RectCells)],
    mover: Id,
    dir: Dir,
    snapshot: &[(Id, RectCells)],
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

fn snapshot_of<Id: Copy + Eq>(snapshot: &[(Id, RectCells)], id: Id) -> Option<RectCells> {
    snapshot.iter().find(|(i, _)| *i == id).map(|(_, c)| *c)
}

pub(crate) fn overlaps(a: RectCells, b: RectCells) -> bool {
    a.page == b.page
        && a.col < b.col + b.col_span
        && b.col < a.col + a.col_span
        && a.row < b.row + b.row_span
        && b.row < a.row + a.row_span
}

pub(crate) fn resize_preview(
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

pub(crate) fn nearest_cell(pos: f64, origin: f64, pitch: f64, max: u32) -> u32 {
    if pitch <= 0.0 {
        return 0;
    }
    let idx = ((pos - origin) / pitch).round() as i32;
    idx.clamp(0, max as i32) as u32
}

pub(crate) fn snap_span(length: f64, cell: f64, pitch: f64, max: u32) -> u32 {
    if pitch <= 0.0 || max == 0 {
        return 1;
    }
    let span = ((length - cell / 2.0) / pitch).floor() as i32 + 1;
    span.clamp(1, max as i32) as u32
}

pub(crate) struct GridMetrics {
    pub columns: u32,
    pub rows: u32,
    pub cw: i32,
    pub ch: i32,
}

pub(crate) fn resize_cells(
    origin: RectCells,
    origin_rect: Rectangle<i32, Logical>,
    handle: Handle,
    pos: Point<f64, Logical>,
    grid: GridMetrics,
) -> RectCells {
    let mut next = origin;
    let pitch_x = (grid.cw + GAP) as f64;
    let pitch_y = (grid.ch + GAP) as f64;
    if handle.east() {
        next.col_span = snap_span(
            pos.x - origin_rect.loc.x as f64,
            grid.cw as f64,
            pitch_x,
            grid.columns.saturating_sub(origin.col),
        );
    }
    if handle.south() {
        next.row_span = snap_span(
            pos.y - origin_rect.loc.y as f64,
            grid.ch as f64,
            pitch_y,
            grid.rows.saturating_sub(origin.row),
        );
    }
    if handle.west() {
        let right = origin.col + origin.col_span;
        let span = snap_span(
            (origin_rect.loc.x + origin_rect.size.w) as f64 - pos.x,
            grid.cw as f64,
            pitch_x,
            right.min(grid.columns),
        );
        next.col = right.saturating_sub(span);
        next.col_span = span;
    }
    if handle.north() {
        let bottom = origin.row + origin.row_span;
        let span = snap_span(
            (origin_rect.loc.y + origin_rect.size.h) as f64 - pos.y,
            grid.ch as f64,
            pitch_y,
            bottom.min(grid.rows),
        );
        next.row = bottom.saturating_sub(span);
        next.row_span = span;
    }
    next.col_span = next
        .col_span
        .min(grid.columns.saturating_sub(next.col))
        .max(1);
    next.row_span = next.row_span.min(grid.rows.saturating_sub(next.row)).max(1);
    next
}
