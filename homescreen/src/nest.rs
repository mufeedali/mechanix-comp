//! In-process Smithay nest: socket, backend, tile crop, nest seat inject.

use std::collections::HashSet;
use std::time::Duration;

use compositor::backend::Backend;
use compositor::state::{SocketName, State};
use smithay::backend::egl::EGLContext;
use smithay::backend::input::{ButtonState, InputTime, KeyState, Keycode};
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::desktop::{Window, WindowSurfaceType};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::MotionEvent;
use smithay::output::Output;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER};
use tracing::info;

use crate::home::{Home, HomeAction, TouchPhase};

pub(crate) struct NestBackend {
    pub nest_gl: GlesRenderer,
    pub damage_tracker: Option<OutputDamageTracker>,
    pub pending_render: bool,
    pub home: Home,
    pub slot_tex: Option<GlesTexture>,
}

impl Backend for NestBackend {
    fn renderer(&mut self) -> &mut GlesRenderer {
        &mut self.nest_gl
    }

    fn seat_name(&self) -> String {
        "nest".to_string()
    }

    fn reset_buffers(&mut self, _output: &Output) {}

    fn schedule_render(&mut self, _output: &Output) {
        self.pending_render = true;
    }
}

impl Drop for NestBackend {
    fn drop(&mut self) {
        if let Some(tex) = self.slot_tex.take() {
            std::mem::forget(tex);
        }
    }
}

pub(crate) struct Nest {
    pub event_loop: EventLoop<'static, State<NestBackend>>,
    pub state: State<NestBackend>,
}

impl Nest {
    pub fn new(nest_gl: GlesRenderer) -> Result<Self, Box<dyn std::error::Error>> {
        let mut event_loop: EventLoop<State<NestBackend>> = EventLoop::try_new()?;
        let display: Display<State<NestBackend>> = Display::new()?;
        let mut state = State::new_with_socket(
            &mut event_loop,
            display,
            NestBackend {
                nest_gl,
                damage_tracker: None,
                pending_render: false,
                home: Home::load()?,
                slot_tex: None,
            },
            SocketName::Nest,
        );
        state.seat.add_touch();
        state.backend_data.home.pump_spawns(&state.socket_name);

        let dmabuf_formats = state.backend_data.renderer().dmabuf_formats();
        let dmabuf_global = state
            .dmabuf_state
            .create_global::<State<NestBackend>>(&state.display_handle, dmabuf_formats);
        state.dmabuf_global = Some(dmabuf_global);

        info!(
            socket = ?state.socket_name,
            "nest listening (set WAYLAND_DISPLAY to this for widgets)"
        );

        Ok(Self { event_loop, state })
    }

    pub fn dispatch(&mut self) {
        let _ = self.event_loop.dispatch(Duration::ZERO, &mut self.state);
    }

    pub fn idle(&mut self) {
        let dropped = self.state.on_idle();
        for surface in &dropped {
            self.state.backend_data.home.release(surface);
        }
        sync_claims(&mut self.state);
        configure_slots(&mut self.state);
        self.state.backend_data.home.reap();
        self.state
            .backend_data
            .home
            .pump_spawns(&self.state.socket_name);
    }
}

pub(crate) fn nest_gl_from_current() -> Result<GlesRenderer, Box<dyn std::error::Error>> {
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_4>::load_required()? };
    let display = egl.get_current_display().ok_or("no current EGL display")?;
    let context = egl.get_current_context().ok_or("no current EGL context")?;
    let cfg_id = egl.query_context(display, context, khronos_egl::CONFIG_ID)?;
    let config = egl
        .choose_first_config(
            display,
            &[khronos_egl::CONFIG_ID, cfg_id, khronos_egl::NONE],
        )?
        .ok_or("no EGL config")?;
    let smithay_ctx = unsafe {
        EGLContext::from_raw(
            display.as_ptr() as *const _,
            config.as_ptr() as *const _,
            context.as_ptr() as *const _,
        )?
    };
    let renderer = unsafe { GlesRenderer::new(smithay_ctx)? };
    info!("Smithay GLES renderer sharing mecha-wayland EGL context");
    Ok(renderer)
}

pub(crate) fn nest_elements(
    state: &mut State<NestBackend>,
    output: &Output,
) -> Vec<CropRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>> {
    let scale = output.current_scale().fractional_scale();
    let origin = state
        .space
        .output_geometry(output)
        .map(|g| g.loc)
        .unwrap_or((0, 0).into());
    let tiles: Vec<_> = state
        .backend_data
        .home
        .widgets_on_screen()
        .into_iter()
        .filter_map(|surface| {
            let clip = state.backend_data.home.slot_rect(&surface)?;
            let ws = state.toplevels.get(&surface)?;
            let loc = state.space.element_location(&ws.window)?;
            Some((
                ws.window.clone(),
                loc - origin,
                clip.loc - origin,
                clip.size,
            ))
        })
        .collect();
    let renderer = &mut state.backend_data.nest_gl;
    let mut elements = Vec::new();
    for (window, loc, clip_loc, clip_size) in &tiles {
        let crop = Rectangle::new(*clip_loc, *clip_size).to_physical_precise_round(scale);
        for elem in window.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
            renderer,
            loc.to_physical_precise_round(scale),
            smithay::utils::Scale::from(scale),
            1.0,
        ) {
            if let Some(cropped) = CropRenderElement::from_element(elem, scale, crop) {
                elements.push(cropped);
            }
        }
    }
    elements
}

pub(crate) fn apply_home_action(state: &mut State<NestBackend>, action: HomeAction) {
    if let Some(id) = state.backend_data.home.take_touch_steal() {
        nest_touch_cancel(state, id);
    }
    match action {
        HomeAction::None => {}
        HomeAction::Redraw => state.schedule_render(),
        HomeAction::Relocate => {
            for surface in state.backend_data.home.widgets_on_screen() {
                let Some(geo) = state.backend_data.home.slot_rect(&surface) else {
                    continue;
                };
                let Some(ws) = state.toplevels.get(&surface) else {
                    continue;
                };
                state.space.relocate_element(&ws.window, geo.loc);
            }
            raise_lifted(state);
            state.schedule_render();
        }
        HomeAction::Relayout => {
            configure_slots(state);
            state.schedule_render();
        }
        HomeAction::PointerClick { pos } => {
            tap_widget_pointer(state, pos);
            state.schedule_render();
        }
        HomeAction::Touch { id, pos, phase } => {
            match phase {
                TouchPhase::Down => nest_touch_down(state, pos, id),
                TouchPhase::Motion => nest_touch_motion(state, pos, id),
                TouchPhase::Up => nest_touch_up(state, pos, id),
            }
            state.schedule_render();
        }
        HomeAction::Launch(exec) => state.backend_data.home.launch(&exec),
        HomeAction::CloseSurface(surface) => {
            if let Some(toplevel) = state
                .toplevels
                .get(&surface)
                .and_then(|ws| ws.window.toplevel().cloned())
            {
                toplevel.send_close();
            }
            state.schedule_render();
        }
        HomeAction::Reload { close } => {
            for surface in close {
                if let Some(toplevel) = state
                    .toplevels
                    .get(&surface)
                    .and_then(|ws| ws.window.toplevel().cloned())
                {
                    toplevel.send_close();
                }
            }
            sync_claims(state);
            configure_slots(state);
            state.schedule_render();
        }
    }
}

fn sync_claims(state: &mut State<NestBackend>) {
    let live: HashSet<WlSurface> = state.toplevels.keys().cloned().collect();
    for surface in &live {
        state.backend_data.home.claim(surface);
    }
    for surface in state.backend_data.home.mapped_widgets() {
        if !live.contains(&surface) {
            state.backend_data.home.release(&surface);
        }
    }
}

fn configure_slots(state: &mut State<NestBackend>) {
    for surface in state.backend_data.home.widgets_on_screen() {
        let Some(geo) = state.backend_data.home.slot_rect(&surface) else {
            continue;
        };
        let Some(ws) = state.toplevels.get(&surface) else {
            continue;
        };
        let Some(toplevel) = ws.window.toplevel().cloned() else {
            continue;
        };
        let window = ws.window.clone();
        let mapped = ws.mapped;
        toplevel.with_pending_state(|pending| {
            pending.size = Some(geo.size);
            pending.states.set(
                smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized,
            );
        });
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        } else {
            toplevel.send_configure();
        }
        if mapped {
            state.space.relocate_element(&window, geo.loc);
        } else {
            state.space.map_element(window.clone(), geo.loc, false);
            if let Some(ws) = state.toplevels.get_mut(&surface) {
                ws.mapped = true;
            }
            if state.active_fullscreen_window().is_none() {
                state.focus_window(&window, SERIAL_COUNTER.next_serial());
            }
        }
    }
    raise_lifted(state);
}

fn nest_window_at(state: &State<NestBackend>, pos: Point<f64, Logical>) -> Option<Window> {
    let p = (pos.x.round() as i32, pos.y.round() as i32);
    for window in state.space.elements().rev() {
        let Some(surface) = window.toplevel().map(|t| t.wl_surface().clone()) else {
            continue;
        };
        if state
            .backend_data
            .home
            .slot_rect(&surface)
            .is_some_and(|geo| geo.contains(p))
        {
            return Some(window.clone());
        }
    }
    None
}

fn raise_lifted(state: &mut State<NestBackend>) {
    if let Some(surface) = state.backend_data.home.lifted_surface()
        && let Some(ws) = state.toplevels.get(&surface)
    {
        state.space.raise_element(&ws.window, false);
    }
}

type NestHit = (Option<Window>, Option<(WlSurface, Point<f64, Logical>)>);

fn nest_under(state: &State<NestBackend>, pos: Point<f64, Logical>) -> NestHit {
    let window = nest_window_at(state, pos);
    let under = window.as_ref().and_then(|window| {
        let loc = state.space.element_location(window)?;
        window
            .surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL)
            .map(|(surface, local)| (surface, (local + loc).to_f64()))
    });
    (window, under)
}

fn tap_widget_pointer(state: &mut State<NestBackend>, pos: Point<f64, Logical>) {
    let serial = SERIAL_COUNTER.next_serial();
    let (window, under) = nest_under(state, pos);
    if let Some(window) = window.as_ref() {
        state.focus_window(window, serial);
    }
    let pointer = state.pointer.clone();
    pointer.motion(
        state,
        under,
        &MotionEvent {
            location: pos,
            serial,
            time: smithay::backend::input::InputTime::now(),
        },
    );
    pointer.button(
        state,
        &smithay::input::pointer::ButtonEvent {
            serial,
            time: smithay::backend::input::InputTime::now(),
            button: 0x110,
            state: ButtonState::Pressed,
        },
    );
    pointer.button(
        state,
        &smithay::input::pointer::ButtonEvent {
            serial: SERIAL_COUNTER.next_serial(),
            time: smithay::backend::input::InputTime::now(),
            button: 0x110,
            state: ButtonState::Released,
        },
    );
    pointer.frame(state);
}

fn nest_touch_down(state: &mut State<NestBackend>, pos: Point<f64, Logical>, id: i32) {
    let Some(touch) = state.seat.get_touch() else {
        return;
    };
    let serial = SERIAL_COUNTER.next_serial();
    let (window, under) = nest_under(state, pos);
    if let Some(window) = window.as_ref() {
        state.focus_window(window, serial);
    }
    touch.down(
        state,
        under,
        &smithay::input::touch::DownEvent {
            slot: touch_slot(id),
            location: pos,
            serial,
            time: smithay::backend::input::InputTime::now(),
        },
    );
    touch.frame(state);
}

fn nest_touch_motion(state: &mut State<NestBackend>, pos: Point<f64, Logical>, id: i32) {
    let Some(touch) = state.seat.get_touch() else {
        return;
    };
    let (_, under) = nest_under(state, pos);
    touch.motion(
        state,
        under,
        &smithay::input::touch::MotionEvent {
            slot: touch_slot(id),
            location: pos,
            time: smithay::backend::input::InputTime::now(),
        },
    );
    touch.frame(state);
}

fn nest_touch_up(state: &mut State<NestBackend>, _pos: Point<f64, Logical>, id: i32) {
    let Some(touch) = state.seat.get_touch() else {
        return;
    };
    touch.up(
        state,
        &smithay::input::touch::UpEvent {
            slot: touch_slot(id),
            serial: SERIAL_COUNTER.next_serial(),
            time: smithay::backend::input::InputTime::now(),
        },
    );
    touch.frame(state);
}

fn nest_touch_cancel(state: &mut State<NestBackend>, _id: i32) {
    let Some(touch) = state.seat.get_touch() else {
        return;
    };
    touch.cancel(state);
    touch.frame(state);
}

fn touch_slot(id: i32) -> smithay::backend::input::TouchSlot {
    smithay::backend::input::TouchSlot::from(u32::try_from(id).ok())
}

pub(crate) struct NestKeyboard {
    pressed: HashSet<u32>,
}

impl NestKeyboard {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
        }
    }

    pub fn key(&mut self, state: &mut State<NestBackend>, evdev: u32, pressed: bool) {
        if pressed {
            if !self.pressed.insert(evdev) {
                return;
            }
        } else if !self.pressed.remove(&evdev) {
            return;
        }
        inject_key(state, evdev, pressed);
    }

    pub fn enter(&mut self, state: &mut State<NestBackend>, keys: &[u8]) {
        for chunk in keys.as_chunks::<4>().0 {
            let evdev = u32::from_ne_bytes(*chunk);
            self.key(state, evdev, true);
        }
    }

    pub fn leave(&mut self, state: &mut State<NestBackend>) {
        let keys: Vec<u32> = self.pressed.drain().collect();
        for evdev in keys {
            inject_key(state, evdev, false);
        }
    }
}

fn inject_key(state: &mut State<NestBackend>, evdev: u32, pressed: bool) {
    let Some(keyboard) = state.seat.get_keyboard() else {
        return;
    };
    let key_state = if pressed {
        KeyState::Pressed
    } else {
        KeyState::Released
    };
    keyboard.input(
        state,
        Keycode::new(evdev + 8),
        key_state,
        SERIAL_COUNTER.next_serial(),
        InputTime::now(),
        |_, _, _| FilterResult::<()>::Forward,
    );
}
