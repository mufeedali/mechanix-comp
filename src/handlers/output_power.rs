//! `zwlr_output_power_manager_v1`: clients (`wlopm`, a future shell) and the
//! power key share [`State::set_output_power`].

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use smithay::desktop::layer_map_for_output;
use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::output_power_management::v1::server::{
    zwlr_output_power_manager_v1::{self, ZwlrOutputPowerManagerV1},
    zwlr_output_power_v1::{self, ZwlrOutputPowerV1},
};
use smithay::reexports::wayland_server::WEnum;
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Logical, Point};
use smithay::wayland::{Dispatch2, GlobalDispatch2};
use tracing::warn;

use crate::backend::Backend;
use crate::state::State;

const VERSION: u32 = 1;

#[derive(Default)]
pub struct OutputPowerManagerUdata;

/// Per-handle output, so `set_mode` knows which CRTC to toggle.
/// `None` means the `wl_output` was already gone; the handle is failed.
pub struct OutputPowerUdata {
    output: Option<Output>,
}

#[derive(Default)]
pub struct OutputPowerManagerState {
    /// Outputs currently DPMS-off. Missing means on.
    off: HashSet<Output>,
    handles: HashMap<Output, HashSet<ZwlrOutputPowerV1>>,
}

impl OutputPowerManagerState {
    pub fn new<D>(dh: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<ZwlrOutputPowerManagerV1, OutputPowerManagerUdata>
            + Dispatch<ZwlrOutputPowerManagerV1, OutputPowerManagerUdata>
            + Dispatch<ZwlrOutputPowerV1, OutputPowerUdata>
            + 'static,
    {
        dh.create_global::<D, ZwlrOutputPowerManagerV1, _>(VERSION, OutputPowerManagerUdata);
        Self::default()
    }

    pub fn any_off(&self) -> bool {
        !self.off.is_empty()
    }

    pub fn is_off(&self, output: &Output) -> bool {
        self.off.contains(output)
    }

    fn send_mode(&self, output: &Output, on: bool) {
        let mode = if on {
            zwlr_output_power_v1::Mode::On
        } else {
            zwlr_output_power_v1::Mode::Off
        };
        if let Some(handles) = self.handles.get(output) {
            for handle in handles {
                handle.mode(mode);
            }
        }
    }

    fn remove_handle(&mut self, resource: &ZwlrOutputPowerV1) {
        for handles in self.handles.values_mut() {
            handles.remove(resource);
        }
        self.handles.retain(|_, handles| !handles.is_empty());
    }

    /// Output left the space: fail every handle and drop the off-bit.
    pub fn output_removed(&mut self, output: &Output) {
        if let Some(handles) = self.handles.remove(output) {
            for handle in handles {
                handle.failed();
            }
        }
        self.off.remove(output);
    }
}

impl<BackendData: Backend + 'static> State<BackendData> {
    /// Apply DPMS for `output` and notify protocol clients. No-op if already
    /// in that state. Backend refusal sends `failed` on that output's handles.
    pub fn set_output_power(&mut self, output: &Output, on: bool) {
        if self.output_power.is_off(output) == !on {
            return;
        }

        if !self.backend_data.set_output_dpms(output, on) {
            warn!(output = %output.name(), on, "output power change failed");
            if let Some(handles) = self.output_power.handles.get(output) {
                for handle in handles {
                    handle.failed();
                }
            }
            return;
        }

        if on {
            self.output_power.off.remove(output);
            self.backend_data.schedule_render(output);
        } else {
            self.output_power.off.insert(output.clone());
        }
        self.output_power.send_mode(output, on);
    }

    /// Power key: if any output is on, blank all; otherwise wake all.
    pub fn toggle_output_power(&mut self) {
        let outputs: Vec<Output> = self.space.outputs().cloned().collect();
        let turn_on = self.output_power.any_off();
        for output in &outputs {
            self.set_output_power(output, turn_on);
        }
    }

    /// Wake every blanked output. Returns true if anything was off (caller
    /// should consume the input that woke the panel).
    pub fn wake_outputs_if_off(&mut self) -> bool {
        if !self.output_power.any_off() {
            return false;
        }
        let outputs: Vec<Output> = self.output_power.off.iter().cloned().collect();
        for output in &outputs {
            self.set_output_power(&output, true);
        }
        true
    }

    /// # SHELL-HELL: compositor owns the power key; logind ignores it.
    pub fn begin_power_press(&mut self) {
        self.cancel_power_timer();
        self.power_long_fired = false;
        let token = self
            .loop_handle
            .insert_source(
                smithay::reexports::calloop::timer::Timer::from_duration(Duration::from_millis(
                    800,
                )),
                |_, _, state| {
                    state.power_timer = None;
                    state.power_long_fired = true;
                    let _ = state.wake_outputs_if_off();
                    state.spawn_power_menu();
                    smithay::reexports::calloop::timer::TimeoutAction::Drop
                },
            )
            .ok();
        self.power_timer = token;
    }

    pub fn end_power_press(&mut self) {
        self.cancel_power_timer();
        if self.power_long_fired {
            self.power_long_fired = false;
            return;
        }
        if self.output_power.any_off() {
            let _ = self.wake_outputs_if_off();
            return;
        }
        // # SHELL-HELL: phone short-press locks then blanks. Wake shows gtklock.
        if !self.is_locked {
            self.spawn_locker();
        }
        let outputs: Vec<Output> = self.space.outputs().cloned().collect();
        for output in &outputs {
            self.set_output_power(output, false);
        }
    }

    /// # SHELL-HELL: session locker spawned from the compositor.
    fn spawn_locker(&mut self) {
        match std::process::Command::new("gtklock")
            .arg("-d")
            .envs(self.socket_name.to_str().map(|v| ("WAYLAND_DISPLAY", v)))
            .spawn()
        {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(err) => warn!("failed to spawn gtklock: {err}"),
        }
    }

    fn cancel_power_timer(&mut self) {
        if let Some(token) = self.power_timer.take() {
            self.loop_handle.remove(token);
        }
    }

    /// # SHELL-HELL: compositor spawns nwg-bar as the power menu.
    fn spawn_power_menu(&mut self) {
        if self.dismiss_power_menu() {
            return;
        }
        // # SHELL-HELL: -p left is a vertical column; -ml 200 recentres it.
        let template = if self.is_locked {
            "bar-locked.json"
        } else {
            "bar.json"
        };
        match std::process::Command::new("nwg-bar")
            .args([
                "-i",
                "48",
                "-p",
                "left",
                "-a",
                "middle",
                "-ml",
                "200",
                "-g",
                "Adwaita-dark",
                "-t",
                template,
            ])
            .envs(self.socket_name.to_str().map(|v| ("WAYLAND_DISPLAY", v)))
            .spawn()
        {
            Ok(mut child) => {
                self.power_menu_live = true;
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(err) => warn!("failed to spawn nwg-bar: {err}"),
        }
    }

    /// # SHELL-HELL: nwg-bar has no dismiss protocol; we pkill it.
    pub fn dismiss_power_menu(&mut self) -> bool {
        let up = self.power_menu_live
            || std::process::Command::new("pgrep")
                .args(["-x", "nwg-bar"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
        self.power_menu_live = false;
        if !up {
            return false;
        }
        let _ = std::process::Command::new("pkill")
            .args(["-x", "nwg-bar"])
            .status();
        true
    }

    /// # SHELL-HELL: tap that missed the power-menu layer dismisses it.
    pub fn dismiss_power_menu_if_outside(&mut self, pos: Point<f64, Logical>) -> bool {
        if !self.power_menu_live {
            return false;
        }
        let on_menu = self.space.outputs().next().is_some_and(|output| {
            let Some(output_geo) = self.space.output_geometry(output) else {
                return false;
            };
            let map = layer_map_for_output(output);
            map.layers().any(|layer| {
                if layer.layer() != smithay::wayland::shell::wlr_layer::Layer::Overlay {
                    return false;
                }
                let Some(geo) = map.layer_geometry(layer) else {
                    return false;
                };
                geo.to_f64().contains(pos - output_geo.loc.to_f64())
            })
        });
        if on_menu {
            return false;
        }
        self.dismiss_power_menu();
        true
    }

    /// DRM/CRTC restore after VT activate. Keeps DPMS-off if we blanked.
    pub fn resume_drm_session(&mut self) {
        self.backend_data.prepare_resume();
        let outputs: Vec<Output> = self.space.outputs().cloned().collect();
        for output in &outputs {
            self.backend_data.reset_buffers(&output);
            if self.output_power.is_off(&output) {
                let _ = self.backend_data.set_output_dpms(&output, false);
            } else {
                self.backend_data.schedule_render(&output);
            }
        }
    }
}

impl<BackendData: Backend + 'static> GlobalDispatch2<ZwlrOutputPowerManagerV1, State<BackendData>>
    for OutputPowerManagerUdata
{
    fn bind(
        &self,
        _state: &mut State<BackendData>,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrOutputPowerManagerV1>,
        data_init: &mut DataInit<'_, State<BackendData>>,
    ) {
        data_init.init(resource, OutputPowerManagerUdata);
    }
}

impl<BackendData: Backend + 'static> Dispatch2<ZwlrOutputPowerManagerV1, State<BackendData>>
    for OutputPowerManagerUdata
{
    fn request(
        &self,
        state: &mut State<BackendData>,
        _client: &Client,
        _resource: &ZwlrOutputPowerManagerV1,
        request: zwlr_output_power_manager_v1::Request,
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, State<BackendData>>,
    ) {
        match request {
            zwlr_output_power_manager_v1::Request::GetOutputPower { id, output } => {
                create_output_power(state, id, &output, data_init);
            }
            zwlr_output_power_manager_v1::Request::Destroy => {}
            _ => unreachable!(),
        }
    }

    fn destroyed(
        &self,
        _state: &mut State<BackendData>,
        _client: ClientId,
        _resource: &ZwlrOutputPowerManagerV1,
    ) {
    }
}

fn create_output_power<BackendData: Backend + 'static>(
    state: &mut State<BackendData>,
    id: New<ZwlrOutputPowerV1>,
    wl_output: &WlOutput,
    data_init: &mut DataInit<'_, State<BackendData>>,
) {
    let Some(output) = Output::from_resource(wl_output) else {
        let handle = data_init.init(id, OutputPowerUdata { output: None });
        handle.failed();
        return;
    };

    if !state.backend_data.output_power_supported(&output) {
        let handle = data_init.init(
            id,
            OutputPowerUdata {
                output: Some(output),
            },
        );
        handle.failed();
        return;
    }

    let on = !state.output_power.is_off(&output);
    let handle = data_init.init(
        id,
        OutputPowerUdata {
            output: Some(output.clone()),
        },
    );
    handle.mode(if on {
        zwlr_output_power_v1::Mode::On
    } else {
        zwlr_output_power_v1::Mode::Off
    });
    state
        .output_power
        .handles
        .entry(output)
        .or_default()
        .insert(handle);
}

impl<BackendData: Backend + 'static> Dispatch2<ZwlrOutputPowerV1, State<BackendData>>
    for OutputPowerUdata
{
    fn request(
        &self,
        state: &mut State<BackendData>,
        _client: &Client,
        resource: &ZwlrOutputPowerV1,
        request: zwlr_output_power_v1::Request,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, State<BackendData>>,
    ) {
        match request {
            zwlr_output_power_v1::Request::SetMode { mode } => {
                let on = match mode {
                    WEnum::Value(zwlr_output_power_v1::Mode::On) => true,
                    WEnum::Value(zwlr_output_power_v1::Mode::Off) => false,
                    WEnum::Value(_) | WEnum::Unknown(_) => {
                        resource.post_error(
                            zwlr_output_power_v1::Error::InvalidMode,
                            "invalid output power mode",
                        );
                        return;
                    }
                };
                let Some(output) = &self.output else {
                    resource.failed();
                    return;
                };
                state.set_output_power(output, on);
            }
            zwlr_output_power_v1::Request::Destroy => {}
            _ => unreachable!(),
        }
    }

    fn destroyed(
        &self,
        state: &mut State<BackendData>,
        _client: ClientId,
        resource: &ZwlrOutputPowerV1,
    ) {
        state.output_power.remove_handle(resource);
    }
}
