use crate::backend::Backend;
use crate::state::State;
use smithay::desktop::{PopupKind, PopupManager};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Rectangle, Serial};
use smithay::wayland::input_method::{InputMethodHandler, PopupSurface};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::selection::data_device::set_data_device_focus;
use smithay::wayland::text_input::TextInputSeat;
use tracing::warn;

impl<BackendData: Backend + 'static> SeatHandler for State<BackendData> {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State<BackendData>> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let client = focused.and_then(|s| self.display_handle.get_client(s.id()).ok());
        set_data_device_focus(&self.display_handle, seat, client);
        // smithay only invokes this with Some (set_focus(None) skips it); leave the previous surface first.
        if let Some(surface) = focused {
            self.sync_text_input_focus(Some(surface));
        }
    }
}

impl<BackendData: Backend + 'static> State<BackendData> {
    /// `leave` the previous text-input, then `enter` `focused` if any. Call before `set_focus(None)`.
    pub fn sync_text_input_focus(&self, focused: Option<&WlSurface>) {
        let text_input = self.seat.text_input();
        if text_input.focus().as_ref() == focused {
            return;
        }
        text_input.leave();
        text_input.set_focus(focused.cloned());
        if focused.is_some() {
            text_input.enter();
        }
    }

    /// Drop keyboard, data-device, and text-input focus. smithay does not call `focus_changed(None)`.
    pub fn clear_keyboard_focus(&mut self, serial: Serial) {
        set_data_device_focus(&self.display_handle, &self.seat, None);
        self.sync_text_input_focus(None);
        self.seat
            .get_keyboard()
            .unwrap()
            .set_focus(self, None, serial);
    }
}

impl<BackendData: Backend> InputMethodHandler for State<BackendData> {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(err) = self.popups.track_popup(PopupKind::from(surface)) {
            warn!("Failed to track popup: {}", err);
        }
    }

    fn popup_repositioned(&mut self, _: PopupSurface) {}

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|parent| parent.surface.clone()) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, smithay::utils::Logical> {
        self.space
            .elements()
            .find_map(|window| {
                (window.wl_surface().as_deref() == Some(parent)).then(|| window.geometry())
            })
            .unwrap_or_default()
    }
}
