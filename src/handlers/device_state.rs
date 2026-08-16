//! phoc's `zphoc_device_state_v1` protocol (vendored bindings in
//! [`phoc_device_state`]).
//!
//! stevia won't consider its Wayland connection "ready" until this global is
//! bound, so we advertise it and report no hardware (no tablet/lid switch and,
//! crucially, no hardware keyboard) so the OSK is allowed to stay active.

use smithay::reexports::wayland_server::{
    Client, DataInit, DisplayHandle, New, Resource, backend::ClientId,
};
use smithay::wayland::{Dispatch2, GlobalDispatch2};

use crate::backend::Backend;
use crate::state::State;

use super::phoc_device_state::{
    zphoc_device_state_v1::{self, ZphocDeviceStateV1},
    zphoc_lid_switch_v1::{self, ZphocLidSwitchV1},
    zphoc_tablet_mode_switch_v1::{self, ZphocTabletModeSwitchV1},
};

pub const DEVICE_STATE_VERSION: u32 = 2;

/// Marker so the device-state global and its resources dispatch here.
#[derive(Default)]
pub struct DeviceStateUdata;

impl<BackendData: Backend + 'static> GlobalDispatch2<ZphocDeviceStateV1, State<BackendData>>
    for DeviceStateUdata
{
    fn bind(
        &self,
        _state: &mut State<BackendData>,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZphocDeviceStateV1>,
        data_init: &mut DataInit<'_, State<BackendData>>,
    ) {
        let device_state = data_init.init(resource, DeviceStateUdata);
        // No tablet/lid switch and no hardware keyboard: the OSK is allowed.
        device_state.capabilities(zphoc_device_state_v1::Capability::empty());
    }
}

impl<BackendData: Backend + 'static> Dispatch2<ZphocDeviceStateV1, State<BackendData>>
    for DeviceStateUdata
{
    fn request(
        &self,
        _state: &mut State<BackendData>,
        _client: &Client,
        resource: &ZphocDeviceStateV1,
        request: zphoc_device_state_v1::Request,
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, State<BackendData>>,
    ) {
        match request {
            // We never advertise tablet/lid capability, so requesting them is a
            // protocol violation; stevia never does, but answer correctly.
            zphoc_device_state_v1::Request::GetTabletModeSwitch { id } => {
                let _ = data_init.init(id, DeviceStateUdata);
                resource.post_error(
                    zphoc_device_state_v1::Error::MissingCapability,
                    "tablet mode switch not supported",
                );
            }
            zphoc_device_state_v1::Request::GetLidSwitch { id } => {
                let _ = data_init.init(id, DeviceStateUdata);
                resource.post_error(
                    zphoc_device_state_v1::Error::MissingCapability,
                    "lid switch not supported",
                );
            }
        }
    }

    fn destroyed(
        &self,
        _state: &mut State<BackendData>,
        _client: ClientId,
        _resource: &ZphocDeviceStateV1,
    ) {
    }
}

impl<BackendData: Backend + 'static> Dispatch2<ZphocTabletModeSwitchV1, State<BackendData>>
    for DeviceStateUdata
{
    fn request(
        &self,
        _state: &mut State<BackendData>,
        _client: &Client,
        _resource: &ZphocTabletModeSwitchV1,
        request: zphoc_tablet_mode_switch_v1::Request,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, State<BackendData>>,
    ) {
        match request {
            zphoc_tablet_mode_switch_v1::Request::Destroy => {}
        }
    }

    fn destroyed(
        &self,
        _state: &mut State<BackendData>,
        _client: ClientId,
        _resource: &ZphocTabletModeSwitchV1,
    ) {
    }
}

impl<BackendData: Backend + 'static> Dispatch2<ZphocLidSwitchV1, State<BackendData>>
    for DeviceStateUdata
{
    fn request(
        &self,
        _state: &mut State<BackendData>,
        _client: &Client,
        _resource: &ZphocLidSwitchV1,
        request: zphoc_lid_switch_v1::Request,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, State<BackendData>>,
    ) {
        match request {
            zphoc_lid_switch_v1::Request::Destroy => {}
        }
    }

    fn destroyed(
        &self,
        _state: &mut State<BackendData>,
        _client: ClientId,
        _resource: &ZphocLidSwitchV1,
    ) {
    }
}
