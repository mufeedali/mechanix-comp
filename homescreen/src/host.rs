//! Host client: layer-shell surface, event loop, one present.

use std::os::fd::AsRawFd;

use app::prelude::*;
use app::{Poll, PrePoll};
use compositor::state::State;
use glow::HasContext;
use io_ring::{IoEvent, IoToken, Ring, RingSettings};
use io_uring::{opcode, types};
use renderer::Renderer;
use renderer::commands::DrawQuad;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesTexture;
use smithay::backend::renderer::{Bind, Texture};
use smithay::desktop::layer_map_for_output;
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::utils::Transform;
use timer::{Relative, Timer, TimerEvent, TimerId};
use tracing::info;
use ui::widgets::Div;
use wayland::{WlPointerButtonState, WlPointerEvent, WlTouchEvent, ZwlrLayerSurfaceV1Event};
use window_manager::{
    WindowHandle, WindowKind, WindowManager, WindowSettings, ZwlrLayerShellV1Layer,
    ZwlrLayerSurfaceV1Anchor, ZwlrLayerSurfaceV1KeyboardInteractivity,
};

use crate::chrome::Fill;
use crate::config::ConfigWatch;
use crate::home::HomeAction;
use crate::nest::{MechaData, Nest, apply_home_action, nest_elements, nest_gl_from_current};

const CLEAR: [f32; 4] = [0.1, 0.1, 0.1, 1.0];

type Chrome = Div<()>;

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
    nest_readable: bool,
    #[lens(skip)]
    config_watch: Option<ConfigWatch>,
    #[lens(skip)]
    config_token: Option<IoToken>,
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
                color_texture: true,
            },
            Div::new(Default::default(), ()),
        );
        let nest = Nest::new(nest_gl)?;
        let config_watch = match ConfigWatch::open(&crate::config::config_path()) {
            Ok(watch) => Some(watch),
            Err(err) => {
                tracing::warn!(%err, "homescreen config watch disabled");
                None
            }
        };
        Ok(Self {
            ring,
            wm,
            timer,
            layer,
            nest,
            quit: false,
            nest_token: None,
            nest_readable: false,
            config_watch,
            config_token: None,
            tick_id: None,
        })
    }

    fn arm_waits(&mut self) {
        if self.nest_token.is_none() {
            let fd = self.nest.event_loop.as_raw_fd();
            let sqe = opcode::PollAdd::new(types::Fd(fd), libc::POLLIN as _).build();
            self.nest_token = Some(self.ring.proxy().push(sqe));
        }
        if self.config_token.is_none()
            && let Some(watch) = &self.config_watch
        {
            let sqe = opcode::PollAdd::new(types::Fd(watch.as_raw_fd()), libc::POLLIN as _).build();
            self.config_token = Some(self.ring.proxy().push(sqe));
        }
    }

    fn apply(&mut self, action: HomeAction) {
        apply_home_action(&mut self.nest.state, action);
        if let Some(wait) = self.nest.state.backend_data.home.take_wait() {
            self.tick_id = Some(self.timer.start_timer(Relative {
                duration: wait,
                repeat: false,
            }));
        }
    }

    fn on_io(&mut self, ev: &IoEvent) {
        let IoEvent::Completed { token, .. } = ev;
        if Some(*token) == self.nest_token {
            self.nest_token = None;
            self.nest_readable = true;
        }
        if Some(*token) == self.config_token {
            self.config_token = None;
        }
    }

    fn on_timer(&mut self, ev: &TimerEvent) {
        if Some(ev.id()) == self.tick_id {
            self.tick_id = None;
        }
    }

    fn on_layer(&mut self, ev: &ZwlrLayerSurfaceV1Event) {
        match ev {
            ZwlrLayerSurfaceV1Event::Configure { width, height, .. } => {
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
                WlPointerEvent::Enter {
                    surface_x,
                    surface_y,
                    ..
                }
                | WlPointerEvent::Motion {
                    surface_x,
                    surface_y,
                    ..
                } => home.pointer_move((*surface_x as f64, *surface_y as f64).into()),
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
        self.apply(action);
    }

    fn on_touch(&mut self, ev: &WlTouchEvent) {
        let action = {
            let home = &mut self.nest.state.backend_data.home;
            match ev {
                WlTouchEvent::Down { id, x, y, .. } => {
                    home.touch_down((*x as f64, *y as f64).into(), *id)
                }
                WlTouchEvent::Motion { x, y, .. } => {
                    home.pointer_move((*x as f64, *y as f64).into())
                }
                WlTouchEvent::Up { .. } | WlTouchEvent::Cancel { .. } => {
                    home.pointer_up(home.last_pos())
                }
                _ => return,
            }
        };
        self.apply(action);
    }

    fn pre_poll(&mut self) {
        if self
            .config_watch
            .as_ref()
            .is_some_and(ConfigWatch::take_changed)
        {
            let action = self.nest.state.backend_data.home.reload();
            self.apply(action);
        }
        if self.nest_readable {
            self.nest_readable = false;
            self.nest.dispatch();
        }
        let action = self.nest.state.backend_data.home.tick();
        self.apply(action);
        self.nest.idle();
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
            let _global = output.create_global::<State<MechaData>>(&self.nest.state.display_handle);
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
        if let Some(old) = self.nest.state.backend_data.slot_tex.take() {
            std::mem::forget(old);
        }
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
        let layer = self.layer;
        let mut render_states = None;

        let Homescreen { wm, nest, .. } = self;
        if wm
            .render_frame(layer, true, |renderer, slot| {
                let gl_id = slot.backend.color_tex_id();
                let backend = &mut nest.state.backend_data;
                if backend
                    .slot_tex
                    .as_ref()
                    .is_none_or(|t| t.tex_id() != gl_id || t.size() != size.into())
                {
                    if let Some(old) = backend.slot_tex.take() {
                        std::mem::forget(old);
                    }
                    backend.slot_tex = Some(unsafe {
                        GlesTexture::from_raw(
                            &backend.nest_gl,
                            Some(glow::RGBA8),
                            false,
                            gl_id,
                            size.into(),
                        )
                    });
                }
                let Some(tex) = backend.slot_tex.as_mut() else {
                    return;
                };
                let Some(tracker) = backend.damage_tracker.as_mut() else {
                    return;
                };
                match backend.nest_gl.bind(tex) {
                    Ok(mut fb) => match tracker.render_output(
                        &mut backend.nest_gl,
                        &mut fb,
                        0,
                        &elements,
                        CLEAR,
                    ) {
                        Ok(res) => render_states = Some(res.states),
                        Err(err) => tracing::error!(%err, "nest render failed"),
                    },
                    Err(err) => tracing::error!(%err, "bind host slot texture failed"),
                }
                let _ = renderer.make_current();
                renderer.active_surface(slot);
                unsafe { renderer.gl.flush() };
                queue_fills(renderer, &fills);
            })
            .is_none()
        {
            return Ok(());
        }

        if let Some(states) = render_states {
            nest.state.update_surface_scanout(&output, &states);
            nest.state.send_frame_callbacks(&output);
            nest.state.backend_data.pending_render = false;
        }
        Ok(())
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
