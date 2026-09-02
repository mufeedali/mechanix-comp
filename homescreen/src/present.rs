//! Host surface is a mecha-wayland window-manager layer. Smithay draws nest
//! tiles into an offscreen texture, we blit that under the WM UI, then attach.

use std::num::NonZeroU32;
use std::os::fd::AsRawFd;
use std::time::Duration;

use app::prelude::*;
use app::{Poll, PrePoll};
use compositor::backend::Backend;
use compositor::state::{SocketName, State};
use glow::HasContext;
use io_ring::{IoEvent, IoToken, Ring, RingSettings};
use io_uring::{opcode, types};
use renderer::commands::DrawQuad;
use renderer::{DmaBuf, RenderableSurface, Renderer};
use smithay::backend::allocator::Fourcc;
use smithay::backend::egl::EGLContext;
use smithay::backend::input::ButtonState;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::{Bind, ImportDma, Offscreen, Texture};
use smithay::desktop::layer_map_for_output;
use smithay::input::pointer::MotionEvent;
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER, Transform};
use timer::{Relative, Timer, TimerEvent, TimerId};
use tracing::info;
use ui::widgets::Div;
use wayland::{
    WlPointerButtonState, WlPointerEvent, WlTouchEvent, ZwlrLayerSurfaceV1Event,
};
use window_manager::{
    WindowHandle, WindowKind, WindowManager, WindowSettings, ZwlrLayerShellV1Layer,
    ZwlrLayerSurfaceV1Anchor, ZwlrLayerSurfaceV1KeyboardInteractivity,
};

use crate::chrome::Fill;
use crate::home::{Home, HomeAction};

const CLEAR: [f32; 4] = [0.1, 0.1, 0.1, 1.0];

type Chrome = Div<()>;

pub struct LayerData {
    nest_gl: GlesRenderer,
    damage_tracker: Option<OutputDamageTracker>,
    pending_render: bool,
    home: Home,
    offscreen: Option<GlesTexture>,
}

impl Backend for LayerData {
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

    fn visible_surfaces(&self) -> Option<Vec<WlSurface>> {
        Some(self.home.visible_surfaces())
    }
}

fn nest_gl_from_current() -> Result<GlesRenderer, Box<dyn std::error::Error>> {
    let egl = unsafe { khronos_egl::DynamicInstance::<khronos_egl::EGL1_4>::load_required()? };
    let display = egl
        .get_current_display()
        .ok_or("no current EGL display")?;
    let context = egl
        .get_current_context()
        .ok_or("no current EGL context")?;
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

fn blit_from_texture(renderer: &Renderer, dst: &RenderableSurface<DmaBuf>, tex_id: u32) -> bool {
    let Some(tex) = NonZeroU32::new(tex_id).map(glow::NativeTexture) else {
        return false;
    };
    let gl = &renderer.gl;
    unsafe {
        let Ok(src) = gl.create_framebuffer() else {
            return false;
        };
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(src));
        gl.framebuffer_texture_2d(
            glow::READ_FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(tex),
            0,
        );
        let ok = gl.check_framebuffer_status(glow::READ_FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE;
        if ok {
            gl.disable(glow::SCISSOR_TEST);
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(dst.fbo));
            gl.blit_framebuffer(
                0,
                0,
                dst.width as i32,
                dst.height as i32,
                0,
                0,
                dst.width as i32,
                dst.height as i32,
                glow::COLOR_BUFFER_BIT,
                glow::NEAREST,
            );
            gl.clear_depth_f32(0.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);
        }
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(dst.fbo));
        gl.delete_framebuffer(src);
        ok
    }
}

fn queue_fills(renderer: &mut Renderer, fills: &[Fill]) {
    for fill in fills {
        renderer.send_command(DrawQuad {
            color: renderer::commands::Color::rgba(
                fill.color[0],
                fill.color[1],
                fill.color[2],
                fill.color[3],
            ),
            border_color: renderer::commands::Color::TRANSPARENT,
            origin: renderer::commands::Point::new(fill.x, fill.y),
            z: 0.5,
            size: renderer::commands::Size::new(fill.w, fill.h),
            border_radius: 0.0,
            border_thickness: 0.0,
            background: renderer::commands::Color::TRANSPARENT,
            is_opaque: false,
        });
    }
}

struct Nest {
    event_loop: EventLoop<'static, State<LayerData>>,
    state: State<LayerData>,
}

#[derive(State)]
struct Homescreen {
    ring: Ring,
    wm: WindowManager,
    timer: Timer,
    #[lens(skip)]
    layer: WindowHandle<Chrome>,
    #[lens(skip)]
    nest: Nest,
    #[lens(skip)]
    quit: bool,
    #[lens(skip)]
    nest_token: Option<IoToken>,
    #[lens(skip)]
    tick_id: Option<TimerId>,
}

impl Homescreen {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let ring = Ring::new(RingSettings::default());
        let mut wm = WindowManager::new(ring.proxy());
        wm.set_auto_present(false);
        wm.renderer().init_pipelines();
        let timer = Timer::new(ring.proxy());
        let nest_gl = nest_gl_from_current()?;
        let layer = wm.spawn_window(
            WindowSettings {
                width: 0,
                height: 0,
                clear_color: window_manager::Color::rgb(0.1, 0.1, 0.1),
                kind: WindowKind::LayerShell {
                    layer: ZwlrLayerShellV1Layer::Background,
                    anchor: ZwlrLayerSurfaceV1Anchor::Top
                        | ZwlrLayerSurfaceV1Anchor::Bottom
                        | ZwlrLayerSurfaceV1Anchor::Left
                        | ZwlrLayerSurfaceV1Anchor::Right,
                    exclusive_zone: -1,
                    namespace: "mechanix-home".into(),
                    keyboard_interactivity: ZwlrLayerSurfaceV1KeyboardInteractivity::None,
                },
                touch_config: None,
                gesture_config: None,
            },
            Div::new(Default::default(), ()),
        );
        let nest = Nest::new(nest_gl)?;
        Ok(Self {
            ring,
            wm,
            timer,
            layer,
            nest,
            quit: false,
            nest_token: None,
            tick_id: None,
        })
    }

    fn arm_waits(&mut self) {
        if self.nest_token.is_none() {
            let fd = self.nest.event_loop.as_raw_fd();
            let sqe = opcode::PollAdd::new(types::Fd(fd), libc::POLLIN as _).build();
            self.nest_token = Some(self.ring.proxy().push(sqe));
        }
        if self.nest.state.backend_data.home.wants_tick() && self.tick_id.is_none() {
            self.tick_id = Some(self.timer.start_timer(Relative {
                duration: Duration::from_millis(16),
                repeat: false,
            }));
        }
    }

    fn on_io(&mut self, ev: &IoEvent) {
        let IoEvent::Completed { token, .. } = ev;
        if Some(*token) == self.nest_token {
            self.nest_token = None;
        }
    }

    fn on_timer(&mut self, ev: &TimerEvent) {
        if Some(ev.id()) == self.tick_id {
            self.tick_id = None;
        }
    }

    fn pump_nest(&mut self) {
        let _ = self
            .nest
            .event_loop
            .dispatch(Duration::ZERO, &mut self.nest.state);
        self.nest.state.on_idle();
        self.nest.state.backend_data.home.reap();
        self.nest
            .state
            .backend_data
            .home
            .pump_spawns(&self.nest.state.socket_name);
    }

    fn on_layer(&mut self, ev: &ZwlrLayerSurfaceV1Event) {
        match ev {
            ZwlrLayerSurfaceV1Event::Configure {
                width, height, ..
            } => {
                if *width > 0 && *height > 0 {
                    info!(w = width, h = height, "layer configure");
                    if let Err(err) = self.apply_host_size(*width as i32, *height as i32) {
                        tracing::warn!(%err, "host resize failed");
                    }
                }
            }
            ZwlrLayerSurfaceV1Event::Closed { .. } => {
                info!("layer surface closed");
                self.quit = true;
            }
        }
    }

    fn on_pointer(&mut self, ev: &WlPointerEvent) {
        let action = {
            let home = &mut self.nest.state.backend_data.home;
            match ev {
                WlPointerEvent::Enter { surface_x, surface_y, .. }
                | WlPointerEvent::Motion { surface_x, surface_y, .. } => {
                    home.pointer_move((*surface_x as f64, *surface_y as f64).into())
                }
                WlPointerEvent::Button {
                    state: WlPointerButtonState::Pressed,
                    ..
                } => home.pointer_down(home.last_pos()),
                WlPointerEvent::Button {
                    state: WlPointerButtonState::Released,
                    ..
                } => home.pointer_up(home.last_pos()),
                _ => return,
            }
        };
        apply_home_action(&mut self.nest.state, action);
    }

    fn on_touch(&mut self, ev: &WlTouchEvent) {
        let action = {
            let home = &mut self.nest.state.backend_data.home;
            match ev {
                WlTouchEvent::Down { x, y, .. } => home.pointer_down((*x as f64, *y as f64).into()),
                WlTouchEvent::Motion { x, y, .. } => home.pointer_move((*x as f64, *y as f64).into()),
                WlTouchEvent::Up { .. } | WlTouchEvent::Cancel { .. } => {
                    home.pointer_up(home.last_pos())
                }
                _ => return,
            }
        };
        apply_home_action(&mut self.nest.state, action);
    }

    fn pre_poll(&mut self) {
        self.pump_nest();
        let action = self.nest.state.backend_data.home.tick();
        apply_home_action(&mut self.nest.state, action);
        if self.nest.state.backend_data.pending_render
            && let Err(err) = self.present()
        {
            tracing::error!(%err, "present failed");
        }
        self.arm_waits();
    }

    fn apply_host_size(&mut self, w: i32, h: i32) -> Result<(), Box<dyn std::error::Error>> {
        self.nest.state.backend_data.home.set_output_size(w, h);
        let mode = Mode {
            size: (w, h).into(),
            refresh: 60_000,
        };
        let output = if let Some(output) = self.nest.state.primary_output() {
            output.change_current_state(Some(mode), None, None, None);
            output
        } else {
            let output = Output::new(
                "homescreen".to_string(),
                PhysicalProperties {
                    size: (0, 0).into(),
                    subpixel: Subpixel::Unknown,
                    make: "mechanix".into(),
                    model: "homescreen".into(),
                    serial_number: "0".into(),
                },
            );
            let _global = output.create_global::<State<LayerData>>(&self.nest.state.display_handle);
            output.change_current_state(
                Some(mode),
                Some(Transform::Normal),
                Some(Scale::Integer(1)),
                Some((0, 0).into()),
            );
            self.nest.state.space.map_output(&output, (0, 0));
            output
        };
        output.set_preferred(mode);
        layer_map_for_output(&output).arrange();
        self.nest.state.backend_data.damage_tracker =
            Some(OutputDamageTracker::from_output(&output));
        self.nest.state.apply_layout(&output);
        self.nest.state.schedule_render();
        Ok(())
    }

    fn present(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.wm.frame_in_flight(self.layer.id()) {
            return Ok(());
        }
        let _ = self.wm.renderer().make_current();
        let Some(output) = self.nest.state.primary_output() else {
            return Ok(());
        };
        let Some(mode) = output.current_mode() else {
            return Ok(());
        };

        let fills = self.nest.state.backend_data.home.fills();
        let elements = nest_elements(&mut self.nest.state, &output);

        let size = (mode.size.w, mode.size.h);
        {
            let backend = &mut self.nest.state.backend_data;
            if backend.offscreen.as_ref().is_none_or(|tex| tex.size() != size.into()) {
                backend.offscreen = Some(backend.nest_gl.create_buffer(Fourcc::Argb8888, size.into())?);
            }
        }

        let (render_states, tex_id) = {
            let backend = &mut self.nest.state.backend_data;
            let tex = backend.offscreen.as_mut().ok_or("offscreen missing")?;
            let tex_id = tex.tex_id();
            let tracker = backend
                .damage_tracker
                .as_mut()
                .ok_or("damage tracker missing")?;
            let renderer = &mut backend.nest_gl;
            let mut fb = renderer.bind(tex)?;
            let result = tracker.render_output(renderer, &mut fb, 0, &elements, CLEAR)?;
            (result.states, tex_id)
        };

        let layer = self.layer;
        if self
            .wm
            .render_frame(layer, true, |renderer, slot| {
                if !blit_from_texture(renderer, slot, tex_id) {
                    tracing::warn!("blit from nest offscreen failed");
                }
                queue_fills(renderer, &fills);
            })
            .is_none()
        {
            return Ok(());
        }

        self.nest
            .state
            .update_surface_scanout(&output, &render_states);
        self.nest.state.send_frame_callbacks(&output);
        self.nest.state.backend_data.pending_render = false;
        Ok(())
    }
}

impl Nest {
    fn new(nest_gl: GlesRenderer) -> Result<Self, Box<dyn std::error::Error>> {
        let mut event_loop: EventLoop<State<LayerData>> = EventLoop::try_new()?;
        let display: Display<State<LayerData>> = Display::new()?;
        let mut state = State::new_with_socket(
            &mut event_loop,
            display,
            LayerData {
                nest_gl,
                damage_tracker: None,
                pending_render: false,
                home: Home::load()?,
                offscreen: None,
            },
            SocketName::Widget,
        );
        state.seat.add_touch();
        state
            .backend_data
            .home
            .pump_spawns(&state.socket_name);

        let dmabuf_formats = state.backend_data.renderer().dmabuf_formats();
        let dmabuf_global = state
            .dmabuf_state
            .create_global::<State<LayerData>>(&state.display_handle, dmabuf_formats);
        state.dmabuf_global = Some(dmabuf_global);

        info!(
            socket = ?state.socket_name,
            "nest listening (set WAYLAND_DISPLAY to this for widgets)"
        );

        Ok(Self { event_loop, state })
    }
}

fn module<S>() -> impl app::RegisteredModule<Homescreen, S> {
    Module::new()
        .on(|s: &mut Homescreen, ev: &ZwlrLayerSurfaceV1Event| s.on_layer(ev))
        .on(|s: &mut Homescreen, ev: &WlPointerEvent| s.on_pointer(ev))
        .on(|s: &mut Homescreen, ev: &WlTouchEvent| s.on_touch(ev))
        .on(|s: &mut Homescreen, ev: &IoEvent| s.on_io(ev))
        .on(|s: &mut Homescreen, ev: &TimerEvent| s.on_timer(ev))
        .on(|s: &mut Homescreen, _: &PrePoll| s.pre_poll())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(Homescreen::new()?)
        .mount(module())
        .mount(io_ring::module())
        .mount(timer::module())
        .mount(window_manager::module());
    app.dispatch(&app::Start);
    while !app.state().quit {
        app.dispatch(&PrePoll);
        app.dispatch(&Poll);
    }
    Ok(())
}

fn nest_elements(
    state: &mut State<LayerData>,
    output: &Output,
) -> Vec<WaylandSurfaceRenderElement<GlesRenderer>> {
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
            let ws = state.toplevels.get(&surface)?;
            let loc = state.space.element_location(&ws.window)?;
            Some((ws.window.clone(), loc - origin))
        })
        .collect();
    let renderer = &mut state.backend_data.nest_gl;
    let mut elements = Vec::new();
    for (window, loc) in &tiles {
        elements.extend(window.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
            renderer,
            loc.to_physical_precise_round(scale),
            smithay::utils::Scale::from(scale),
            1.0,
        ));
    }
    elements
}

fn raise_lifted(state: &mut State<LayerData>) {
    if let Some(surface) = state.backend_data.home.lifted_surface()
        && let Some(ws) = state.toplevels.get(&surface)
    {
        state.space.raise_element(&ws.window, false);
    }
}

fn apply_home_action(state: &mut State<LayerData>, action: HomeAction) {
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
        HomeAction::TapWidget(pos) => {
            tap_widget(state, pos);
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
    }
}

fn tap_widget(state: &mut State<LayerData>, pos: Point<f64, Logical>) {
    let serial = SERIAL_COUNTER.next_serial();
    if let Some(window) = state.space.element_under(pos).map(|(w, _)| w.clone()) {
        state.focus_window(&window, serial);
    }
    let under = state.surface_under(pos);
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
