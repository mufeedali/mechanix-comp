//! In-process Smithay nest: socket, backend, tile crop, tap inject.

use std::time::Duration;

use compositor::backend::Backend;
use compositor::state::{SocketName, State};
use smithay::backend::egl::EGLContext;
use smithay::backend::input::ButtonState;
use smithay::backend::renderer::ImportDma;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::desktop::{Window, WindowSurfaceType};
use smithay::input::pointer::MotionEvent;
use smithay::output::Output;
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER};
use tracing::info;

use crate::home::{Home, HomeAction};

pub(crate) struct MechaData {
    pub nest_gl: GlesRenderer,
    pub damage_tracker: Option<OutputDamageTracker>,
    pub pending_render: bool,
    pub home: Home,
    pub slot_tex: Option<GlesTexture>,
}

impl Backend for MechaData {
    fn renderer(&mut self) -> &mut GlesRenderer {
        &mut self.nest_gl
    }

    fn seat_name(&self) -> String {
        "homescreen".to_string()
    }

    fn reset_buffers(&mut self, _output: &Output) {}

    fn schedule_render(&mut self, _output: &Output) {
        self.pending_render = true;
    }

    fn on_new_toplevel(&mut self, surface: &WlSurface) {
        self.home.claim(surface);
    }

    fn on_unmapped(&mut self, surface: &WlSurface) {
        self.home.release(surface);
    }

    fn placement(&self, surface: &WlSurface) -> Option<Rectangle<i32, Logical>> {
        self.home.placement(surface)
    }
}

impl Drop for MechaData {
    fn drop(&mut self) {
        if let Some(tex) = self.slot_tex.take() {
            std::mem::forget(tex);
        }
    }
}

pub(crate) struct Nest {
    pub event_loop: EventLoop<'static, State<MechaData>>,
    pub state: State<MechaData>,
}

impl Nest {
    pub fn new(nest_gl: GlesRenderer) -> Result<Self, Box<dyn std::error::Error>> {
        let mut event_loop: EventLoop<State<MechaData>> = EventLoop::try_new()?;
        let display: Display<State<MechaData>> = Display::new()?;
        let mut state = State::new_with_socket(
            &mut event_loop,
            display,
            MechaData {
                nest_gl,
                damage_tracker: None,
                pending_render: false,
                home: Home::load()?,
                slot_tex: None,
            },
            SocketName::Widget,
        );
        state.seat.add_touch();
        state.backend_data.home.pump_spawns(&state.socket_name);

        let dmabuf_formats = state.backend_data.renderer().dmabuf_formats();
        let dmabuf_global = state
            .dmabuf_state
            .create_global::<State<MechaData>>(&state.display_handle, dmabuf_formats);
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
        self.state.on_idle();
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
    state: &mut State<MechaData>,
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
        .visible_surfaces()
        .into_iter()
        .filter_map(|surface| {
            let clip = state.backend_data.home.placement(&surface)?;
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

pub(crate) fn apply_home_action(state: &mut State<MechaData>, action: HomeAction) {
    match action {
        HomeAction::None => {}
        HomeAction::Redraw => state.schedule_render(),
        HomeAction::Relocate => {
            for surface in state.backend_data.home.visible_surfaces() {
                let Some(geo) = state.backend_data.home.placement(&surface) else {
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
            if let Some(output) = state.primary_output() {
                state.apply_layout(&output);
            }
            raise_lifted(state);
            state.schedule_render();
        }
        HomeAction::TapWidget { pos, touch_id } => {
            if let Some(id) = touch_id {
                tap_widget_touch(state, pos, id);
            } else {
                tap_widget_pointer(state, pos);
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
            if let Some(output) = state.primary_output() {
                state.apply_layout(&output);
            }
            raise_lifted(state);
            state.schedule_render();
        }
    }
}

fn nest_window_at(state: &State<MechaData>, pos: Point<f64, Logical>) -> Option<Window> {
    let p = (pos.x.round() as i32, pos.y.round() as i32);
    for window in state.space.elements().rev() {
        let Some(surface) = window.toplevel().map(|t| t.wl_surface().clone()) else {
            continue;
        };
        if state
            .backend_data
            .home
            .placement(&surface)
            .is_some_and(|geo| geo.contains(p))
        {
            return Some(window.clone());
        }
    }
    None
}

fn raise_lifted(state: &mut State<MechaData>) {
    if let Some(surface) = state.backend_data.home.lifted_surface()
        && let Some(ws) = state.toplevels.get(&surface)
    {
        state.space.raise_element(&ws.window, false);
    }
}

type NestHit = (Option<Window>, Option<(WlSurface, Point<f64, Logical>)>);

fn nest_under(state: &State<MechaData>, pos: Point<f64, Logical>) -> NestHit {
    let window = nest_window_at(state, pos);
    let under = window.as_ref().and_then(|window| {
        let loc = state.space.element_location(window)?;
        window
            .surface_under(pos - loc.to_f64(), WindowSurfaceType::ALL)
            .map(|(surface, local)| (surface, (local + loc).to_f64()))
    });
    (window, under)
}

fn tap_widget_pointer(state: &mut State<MechaData>, pos: Point<f64, Logical>) {
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
            time: 0,
        },
    );
    pointer.button(
        state,
        &smithay::input::pointer::ButtonEvent {
            serial,
            time: 0,
            button: 0x110,
            state: ButtonState::Pressed,
        },
    );
    pointer.button(
        state,
        &smithay::input::pointer::ButtonEvent {
            serial: SERIAL_COUNTER.next_serial(),
            time: 0,
            button: 0x110,
            state: ButtonState::Released,
        },
    );
    pointer.frame(state);
}

fn tap_widget_touch(state: &mut State<MechaData>, pos: Point<f64, Logical>, id: i32) {
    let Some(touch) = state.seat.get_touch() else {
        tap_widget_pointer(state, pos);
        return;
    };
    let serial = SERIAL_COUNTER.next_serial();
    let (window, under) = nest_under(state, pos);
    if let Some(window) = window.as_ref() {
        state.focus_window(window, serial);
    }
    let slot = smithay::backend::input::TouchSlot::from(u32::try_from(id).ok());
    touch.down(
        state,
        under,
        &smithay::input::touch::DownEvent {
            slot,
            location: pos,
            serial,
            time: 0,
        },
    );
    touch.up(
        state,
        &smithay::input::touch::UpEvent {
            slot,
            serial: SERIAL_COUNTER.next_serial(),
            time: 0,
        },
    );
    touch.frame(state);
}
