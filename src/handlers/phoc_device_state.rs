#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    unused_parens,
    unused_unsafe,
    unreachable_patterns,
    irrefutable_let_patterns,
    dead_code,
    non_camel_case_types,
    non_upper_case_globals,
    clippy::all
)]
use wayland_server;
use wayland_server::protocol::*;

pub mod __interfaces {
    use std::ptr::null;
    use wayland_server::protocol::__interfaces::*;
    struct SyncWrapper<T>(T);
    unsafe impl<T> Sync for SyncWrapper<T> {}
    static types_null: SyncWrapper<[*const ::wayland_backend::protocol::wl_interface; 1]> =
        SyncWrapper([null::<::wayland_backend::protocol::wl_interface>(); 1]);
    pub static ZPHOC_DEVICE_STATE_V1_INTERFACE: ::wayland_backend::protocol::Interface =
        ::wayland_backend::protocol::Interface {
            name: "zphoc_device_state_v1",
            version: 2u32,
            requests: &[
                ::wayland_backend::protocol::MessageDesc {
                    name: "get_tablet_mode_switch",
                    signature: &[::wayland_backend::protocol::ArgumentType::NewId],
                    since: 1u32,
                    is_destructor: false,
                    child_interface: Some(&ZPHOC_TABLET_MODE_SWITCH_V1_INTERFACE),
                    arg_interfaces: &[],
                },
                ::wayland_backend::protocol::MessageDesc {
                    name: "get_lid_switch",
                    signature: &[::wayland_backend::protocol::ArgumentType::NewId],
                    since: 1u32,
                    is_destructor: false,
                    child_interface: Some(&ZPHOC_LID_SWITCH_V1_INTERFACE),
                    arg_interfaces: &[],
                },
            ],
            events: &[::wayland_backend::protocol::MessageDesc {
                name: "capabilities",
                signature: &[::wayland_backend::protocol::ArgumentType::Uint],
                since: 1u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[],
            }],
            c_ptr: Some(unsafe { &zphoc_device_state_v1_interface }),
        };
    static zphoc_device_state_v1_requests_get_tablet_mode_switch_types: SyncWrapper<
        [*const ::wayland_backend::protocol::wl_interface; 1],
    > =
        SyncWrapper([&zphoc_tablet_mode_switch_v1_interface
            as *const ::wayland_backend::protocol::wl_interface]);
    static zphoc_device_state_v1_requests_get_lid_switch_types: SyncWrapper<
        [*const ::wayland_backend::protocol::wl_interface; 1],
    > = SyncWrapper([
        &zphoc_lid_switch_v1_interface as *const ::wayland_backend::protocol::wl_interface
    ]);
    static zphoc_device_state_v1_requests: SyncWrapper<
        [::wayland_backend::protocol::wl_message; 2],
    > = SyncWrapper([
        ::wayland_backend::protocol::wl_message {
            name: b"get_tablet_mode_switch\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"n\0" as *const u8 as *const std::os::raw::c_char,
            types: zphoc_device_state_v1_requests_get_tablet_mode_switch_types
                .0
                .as_ptr(),
        },
        ::wayland_backend::protocol::wl_message {
            name: b"get_lid_switch\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"n\0" as *const u8 as *const std::os::raw::c_char,
            types: zphoc_device_state_v1_requests_get_lid_switch_types
                .0
                .as_ptr(),
        },
    ]);
    static zphoc_device_state_v1_events: SyncWrapper<[::wayland_backend::protocol::wl_message; 1]> =
        SyncWrapper([::wayland_backend::protocol::wl_message {
            name: b"capabilities\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"u\0" as *const u8 as *const std::os::raw::c_char,
            types: types_null.0.as_ptr(),
        }]);
    pub static zphoc_device_state_v1_interface: ::wayland_backend::protocol::wl_interface =
        ::wayland_backend::protocol::wl_interface {
            name: b"zphoc_device_state_v1\0" as *const u8 as *const std::os::raw::c_char,
            version: 2,
            request_count: 2,
            requests: zphoc_device_state_v1_requests.0.as_ptr(),
            event_count: 1,
            events: zphoc_device_state_v1_events.0.as_ptr(),
        };
    pub static ZPHOC_TABLET_MODE_SWITCH_V1_INTERFACE: ::wayland_backend::protocol::Interface =
        ::wayland_backend::protocol::Interface {
            name: "zphoc_tablet_mode_switch_v1",
            version: 1u32,
            requests: &[::wayland_backend::protocol::MessageDesc {
                name: "destroy",
                signature: &[],
                since: 1u32,
                is_destructor: true,
                child_interface: None,
                arg_interfaces: &[],
            }],
            events: &[
                ::wayland_backend::protocol::MessageDesc {
                    name: "disabled",
                    signature: &[],
                    since: 1u32,
                    is_destructor: false,
                    child_interface: None,
                    arg_interfaces: &[],
                },
                ::wayland_backend::protocol::MessageDesc {
                    name: "enabled",
                    signature: &[],
                    since: 1u32,
                    is_destructor: false,
                    child_interface: None,
                    arg_interfaces: &[],
                },
            ],
            c_ptr: Some(unsafe { &zphoc_tablet_mode_switch_v1_interface }),
        };
    static zphoc_tablet_mode_switch_v1_requests: SyncWrapper<
        [::wayland_backend::protocol::wl_message; 1],
    > = SyncWrapper([::wayland_backend::protocol::wl_message {
        name: b"destroy\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    }]);
    static zphoc_tablet_mode_switch_v1_events: SyncWrapper<
        [::wayland_backend::protocol::wl_message; 2],
    > = SyncWrapper([
        ::wayland_backend::protocol::wl_message {
            name: b"disabled\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"\0" as *const u8 as *const std::os::raw::c_char,
            types: types_null.0.as_ptr(),
        },
        ::wayland_backend::protocol::wl_message {
            name: b"enabled\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"\0" as *const u8 as *const std::os::raw::c_char,
            types: types_null.0.as_ptr(),
        },
    ]);
    pub static zphoc_tablet_mode_switch_v1_interface: ::wayland_backend::protocol::wl_interface =
        ::wayland_backend::protocol::wl_interface {
            name: b"zphoc_tablet_mode_switch_v1\0" as *const u8 as *const std::os::raw::c_char,
            version: 1,
            request_count: 1,
            requests: zphoc_tablet_mode_switch_v1_requests.0.as_ptr(),
            event_count: 2,
            events: zphoc_tablet_mode_switch_v1_events.0.as_ptr(),
        };
    pub static ZPHOC_LID_SWITCH_V1_INTERFACE: ::wayland_backend::protocol::Interface =
        ::wayland_backend::protocol::Interface {
            name: "zphoc_lid_switch_v1",
            version: 1u32,
            requests: &[::wayland_backend::protocol::MessageDesc {
                name: "destroy",
                signature: &[],
                since: 1u32,
                is_destructor: true,
                child_interface: None,
                arg_interfaces: &[],
            }],
            events: &[
                ::wayland_backend::protocol::MessageDesc {
                    name: "opened",
                    signature: &[],
                    since: 1u32,
                    is_destructor: false,
                    child_interface: None,
                    arg_interfaces: &[],
                },
                ::wayland_backend::protocol::MessageDesc {
                    name: "closed",
                    signature: &[],
                    since: 1u32,
                    is_destructor: false,
                    child_interface: None,
                    arg_interfaces: &[],
                },
            ],
            c_ptr: Some(unsafe { &zphoc_lid_switch_v1_interface }),
        };
    static zphoc_lid_switch_v1_requests: SyncWrapper<[::wayland_backend::protocol::wl_message; 1]> =
        SyncWrapper([::wayland_backend::protocol::wl_message {
            name: b"destroy\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"\0" as *const u8 as *const std::os::raw::c_char,
            types: types_null.0.as_ptr(),
        }]);
    static zphoc_lid_switch_v1_events: SyncWrapper<[::wayland_backend::protocol::wl_message; 2]> =
        SyncWrapper([
            ::wayland_backend::protocol::wl_message {
                name: b"opened\0" as *const u8 as *const std::os::raw::c_char,
                signature: b"\0" as *const u8 as *const std::os::raw::c_char,
                types: types_null.0.as_ptr(),
            },
            ::wayland_backend::protocol::wl_message {
                name: b"closed\0" as *const u8 as *const std::os::raw::c_char,
                signature: b"\0" as *const u8 as *const std::os::raw::c_char,
                types: types_null.0.as_ptr(),
            },
        ]);
    pub static zphoc_lid_switch_v1_interface: ::wayland_backend::protocol::wl_interface =
        ::wayland_backend::protocol::wl_interface {
            name: b"zphoc_lid_switch_v1\0" as *const u8 as *const std::os::raw::c_char,
            version: 1,
            request_count: 1,
            requests: zphoc_lid_switch_v1_requests.0.as_ptr(),
            event_count: 2,
            events: zphoc_lid_switch_v1_events.0.as_ptr(),
        };
}
use self::__interfaces::*;

#[doc = "Device state information\n\nPhones, tablets, convertibles, laptops can have additional hardware attached or switch\ntheir operation mode from e.g. tablet to laptop. This protocol is meant to provide information\nabout these changes to interested clients.\n\nWarning! The protocol described in this file is experimental and\nbackward incompatible changes may be made. Backward compatible changes\nmay be added together with the corresponding interface version bump.\nBackward incompatible changes are done by bumping the version number in\nthe protocol and interface names and resetting the interface version.\nOnce the protocol is to be declared stable, the 'z' prefix and the\nversion number in the protocol and interface names are removed and the\ninterface version number is reset.\n\nThis protocol is meant to collect necessary bits before we propose an\nupstream solution."]
pub mod zphoc_device_state_v1 {
    use ::wayland_server::{
        Dispatch, DispatchError, DisplayHandle, New, Resource, ResourceData, Weak,
        backend::{
            InvalidId, ObjectData, ObjectId, WeakHandle,
            protocol::{Argument, Interface, Message, WEnum, same_interface},
            smallvec,
        },
    };
    use std::os::unix::io::OwnedFd;
    use std::sync::Arc;
    bitflags::bitflags! {
        /// Device capability bitmask: a member set means the hardware is present.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct Capability: u32 {
            const TabletModeSwitch = 1;
            const LidSwitch = 2;
            const Keyboard = 4;
        }
    }
    impl std::convert::TryFrom<u32> for Capability {
        type Error = ();
        fn try_from(val: u32) -> Result<Capability, ()> {
            Capability::from_bits(val).ok_or(())
        }
    }
    impl std::convert::From<Capability> for u32 {
        fn from(val: Capability) -> u32 {
            val.bits()
        }
    }
    #[doc = "zphoc_device_state_v1 error values\n\nThese errors can be emitted in response to zphoc_device_state_v1 requests."]
    #[repr(u32)]
    #[non_exhaustive]
    #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Error {
        #[doc = "get_tablet_mode_switch called on device without the matching capability"]
        MissingCapability = 0,
    }
    impl std::convert::TryFrom<u32> for Error {
        type Error = ();
        fn try_from(val: u32) -> Result<Error, ()> {
            match val {
                0 => Ok(Error::MissingCapability),
                _ => Err(()),
            }
        }
    }
    impl std::convert::From<Error> for u32 {
        fn from(val: Error) -> u32 {
            val as u32
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_TABLET_MODE_SWITCH_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this request"]
    pub const REQ_GET_TABLET_MODE_SWITCH_OPCODE: u16 = 0u16;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_LID_SWITCH_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this request"]
    pub const REQ_GET_LID_SWITCH_OPCODE: u16 = 1u16;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CAPABILITIES_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this event"]
    pub const EVT_CAPABILITIES_OPCODE: u16 = 0u16;
    #[non_exhaustive]
    #[derive(Debug)]
    pub enum Request {
        #[doc = "return tablet-mode-switch object\n\nThe ID provided will be initialized to the phoc_tablet_mode_switch interface\nfor this device\n\nThis request only takes effect if the seat has the tablet-mode-switch\ncapability, or has had the tablet-mode-switch capability in the past.\nIt is a protocol violation to issue this request on a seat that has\nnever had the tablet-mode-switch capability. The\nmissing_capability error will be sent in this case."]
        GetTabletModeSwitch {
            #[doc = "tablet mode switch"]
            id: New<super::zphoc_tablet_mode_switch_v1::ZphocTabletModeSwitchV1>,
        },

        #[doc = "return tablet-mode-switch object\n\nThe ID provided will be initialized to the phoc_lid_switch interface\nfor this device\n\nThis request only takes effect if the seat has the lid-switch\ncapability, or has had the lid-switch capability in the past.\nIt is a protocol violation to issue this request on a seat that has\nnever had the tablet-mode-switch capability. The\nmissing_capability error will be sent in this case."]
        GetLidSwitch {
            #[doc = "lid switch"]
            id: New<super::zphoc_lid_switch_v1::ZphocLidSwitchV1>,
        },
    }
    impl Request {
        #[doc = "Get the opcode number of this message"]
        pub fn opcode(&self) -> u16 {
            match *self {
                Request::GetTabletModeSwitch { .. } => 0u16,
                Request::GetLidSwitch { .. } => 1u16,
            }
        }
    }
    #[non_exhaustive]
    #[derive(Debug)]
    pub enum Event<'a> {
        #[doc = "The device capabilitiers changed\n\nThis is emitted whenever a device gains or loses a capbility.\nThe argument is a capability enum containing the complete set\nof hw capabilities this device has."]
        Capabilities {
            #[doc = "Hardware capabilities of the device"]
            capabilities: WEnum<Capability>,
        },

        #[doc(hidden)]
        __phantom_lifetime {
            phantom: std::marker::PhantomData<&'a ()>,
            never: std::convert::Infallible,
        },
    }
    impl<'a> Event<'a> {
        #[doc = "Get the opcode number of this message"]
        pub fn opcode(&self) -> u16 {
            match *self {
                Event::Capabilities { .. } => 0u16,
                Event::__phantom_lifetime { never, .. } => match never {},
            }
        }
    }
    #[doc = "Device state information\n\nPhones, tablets, convertibles, laptops can have additional hardware attached or switch\ntheir operation mode from e.g. tablet to laptop. This protocol is meant to provide information\nabout these changes to interested clients.\n\nWarning! The protocol described in this file is experimental and\nbackward incompatible changes may be made. Backward compatible changes\nmay be added together with the corresponding interface version bump.\nBackward incompatible changes are done by bumping the version number in\nthe protocol and interface names and resetting the interface version.\nOnce the protocol is to be declared stable, the 'z' prefix and the\nversion number in the protocol and interface names are removed and the\ninterface version number is reset.\n\nThis protocol is meant to collect necessary bits before we propose an\nupstream solution.\n\nSee also the [Request] enum for this interface."]
    #[derive(Clone, Debug)]
    pub struct ZphocDeviceStateV1 {
        id: ObjectId,
        version: u32,
        data: Option<Arc<dyn std::any::Any + Send + Sync + 'static>>,
        handle: WeakHandle,
    }
    impl std::cmp::PartialEq for ZphocDeviceStateV1 {
        #[inline]
        fn eq(&self, other: &ZphocDeviceStateV1) -> bool {
            self.id == other.id
        }
    }
    impl std::cmp::Eq for ZphocDeviceStateV1 {}
    impl PartialEq<Weak<ZphocDeviceStateV1>> for ZphocDeviceStateV1 {
        #[inline]
        fn eq(&self, other: &Weak<ZphocDeviceStateV1>) -> bool {
            self.id == other.id()
        }
    }
    impl std::borrow::Borrow<ObjectId> for ZphocDeviceStateV1 {
        #[inline]
        fn borrow(&self) -> &ObjectId {
            &self.id
        }
    }
    impl std::hash::Hash for ZphocDeviceStateV1 {
        #[inline]
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.id.hash(state)
        }
    }
    impl ::wayland_server::Resource for ZphocDeviceStateV1 {
        type Request = Request;
        type Event<'event> = Event<'event>;
        #[inline]
        fn interface() -> &'static Interface {
            &super::ZPHOC_DEVICE_STATE_V1_INTERFACE
        }
        #[inline]
        fn id(&self) -> ObjectId {
            self.id.clone()
        }
        #[inline]
        fn version(&self) -> u32 {
            self.version
        }
        #[inline]
        fn data<U: 'static>(&self) -> Option<&U> {
            self.data
                .as_ref()
                .and_then(|arc| (&**arc).downcast_ref::<ResourceData<Self, U>>())
                .map(|data| &data.udata)
        }
        #[inline]
        fn object_data(&self) -> Option<&Arc<dyn std::any::Any + Send + Sync>> {
            self.data.as_ref()
        }
        fn handle(&self) -> &WeakHandle {
            &self.handle
        }
        #[inline]
        fn from_id(conn: &DisplayHandle, id: ObjectId) -> Result<Self, InvalidId> {
            if !same_interface(id.interface(), Self::interface()) && !id.is_null() {
                return Err(InvalidId);
            }
            let version = conn
                .object_info(id.clone())
                .map(|info| info.version)
                .unwrap_or(0);
            let data = conn.get_object_data(id.clone()).ok();
            Ok(ZphocDeviceStateV1 {
                id,
                data,
                version,
                handle: conn.backend_handle().downgrade(),
            })
        }
        fn send_event(&self, evt: Self::Event<'_>) -> Result<(), InvalidId> {
            let handle = DisplayHandle::from(self.handle.upgrade().ok_or(InvalidId)?);
            handle.send_event(self, evt)
        }
        fn parse_request(
            conn: &DisplayHandle,
            msg: Message<ObjectId, OwnedFd>,
        ) -> Result<(Self, Self::Request), DispatchError> {
            let me = Self::from_id(conn, msg.sender_id.clone()).unwrap();
            let mut arg_iter = msg.args.into_iter();
            match msg.opcode {
                0u16 => {
                    if let (Some(Argument::NewId(id))) = (arg_iter.next()) {
                        Ok((me,
                                    Request::GetTabletModeSwitch {
                                        id: New::wrap(match <super::zphoc_tablet_mode_switch_v1::ZphocTabletModeSwitchV1
                                                        as Resource>::from_id(conn, id.clone()) {
                                                Ok(p) => p,
                                                Err(_) =>
                                                    return Err(DispatchError::BadMessage {
                                                                sender_id: msg.sender_id,
                                                                interface: Self::interface().name,
                                                                opcode: msg.opcode,
                                                            }),
                                            }),
                                    }))
                    } else {
                        Err(DispatchError::BadMessage {
                            sender_id: msg.sender_id,
                            interface: Self::interface().name,
                            opcode: msg.opcode,
                        })
                    }
                }
                1u16 => {
                    if let (Some(Argument::NewId(id))) = (arg_iter.next()) {
                        Ok((me,
                                    Request::GetLidSwitch {
                                        id: New::wrap(match <super::zphoc_lid_switch_v1::ZphocLidSwitchV1
                                                        as Resource>::from_id(conn, id.clone()) {
                                                Ok(p) => p,
                                                Err(_) =>
                                                    return Err(DispatchError::BadMessage {
                                                                sender_id: msg.sender_id,
                                                                interface: Self::interface().name,
                                                                opcode: msg.opcode,
                                                            }),
                                            }),
                                    }))
                    } else {
                        Err(DispatchError::BadMessage {
                            sender_id: msg.sender_id,
                            interface: Self::interface().name,
                            opcode: msg.opcode,
                        })
                    }
                }
                _ => Err(DispatchError::BadMessage {
                    sender_id: msg.sender_id,
                    interface: Self::interface().name,
                    opcode: msg.opcode,
                }),
            }
        }
        fn write_event<'a>(
            &self,
            conn: &DisplayHandle,
            msg: Self::Event<'a>,
        ) -> Result<Message<ObjectId, std::os::unix::io::BorrowedFd<'a>>, InvalidId> {
            match msg {
                Event::Capabilities { capabilities } => Ok(Message {
                    sender_id: self.id.clone(),
                    opcode: 0u16,
                    args: {
                        let mut vec = smallvec::SmallVec::new();
                        vec.push(Argument::Uint(capabilities.into()));
                        vec
                    },
                }),
                Event::__phantom_lifetime { never, .. } => match never {},
            }
        }
        fn __set_object_data(
            &mut self,
            odata: std::sync::Arc<dyn std::any::Any + Send + Sync + 'static>,
        ) {
            self.data = Some(odata);
        }
    }
    impl ZphocDeviceStateV1 {
        #[doc = "The device capabilitiers changed\n\nThis is emitted whenever a device gains or loses a capbility.\nThe argument is a capability enum containing the complete set\nof hw capabilities this device has."]
        #[allow(clippy::too_many_arguments)]
        pub fn capabilities(&self, capabilities: Capability) {
            let _ = self.send_event(Event::Capabilities {
                capabilities: WEnum::Value(capabilities),
            });
        }
    }
}
#[doc = "A tablet mode switch\n\nThe tablet_mode_switch interface represents a tablet mode switch.\nIt can have two possible values:\n\nThe wl_pointer interface generates enabled and disabled events to indicate\nswitch state changes"]
pub mod zphoc_tablet_mode_switch_v1 {
    use ::wayland_server::{
        Dispatch, DispatchError, DisplayHandle, New, Resource, ResourceData, Weak,
        backend::{
            InvalidId, ObjectData, ObjectId, WeakHandle,
            protocol::{Argument, Interface, Message, WEnum, same_interface},
            smallvec,
        },
    };
    use std::os::unix::io::OwnedFd;
    use std::sync::Arc;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this request"]
    pub const REQ_DESTROY_OPCODE: u16 = 0u16;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DISABLED_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this event"]
    pub const EVT_DISABLED_OPCODE: u16 = 0u16;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_ENABLED_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this event"]
    pub const EVT_ENABLED_OPCODE: u16 = 1u16;
    #[non_exhaustive]
    pub enum Request {
        #[doc = "release the switch object\n\nUsing this request a client can tell the server that it is not going to\nuse the switch object anymore.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl Request {
        #[doc = "Get the opcode number of this message"]
        pub fn opcode(&self) -> u16 {
            match *self {
                Request::Destroy => 0u16,
            }
        }
    }
    #[non_exhaustive]
    pub enum Event<'a> {
        #[doc = "Tablet mode got disabled"]
        Disabled,

        #[doc = "Tablet mode got enabled"]
        Enabled,

        #[doc(hidden)]
        __phantom_lifetime {
            phantom: std::marker::PhantomData<&'a ()>,
            never: std::convert::Infallible,
        },
    }
    impl<'a> Event<'a> {
        #[doc = "Get the opcode number of this message"]
        pub fn opcode(&self) -> u16 {
            match *self {
                Event::Disabled => 0u16,
                Event::Enabled => 1u16,
                Event::__phantom_lifetime { never, .. } => match never {},
            }
        }
    }
    #[doc = "A tablet mode switch\n\nThe tablet_mode_switch interface represents a tablet mode switch.\nIt can have two possible values:\n\nThe wl_pointer interface generates enabled and disabled events to indicate\nswitch state changes\n\nSee also the [Request] enum for this interface."]
    #[derive(Clone, Debug)]
    pub struct ZphocTabletModeSwitchV1 {
        id: ObjectId,
        version: u32,
        data: Option<Arc<dyn std::any::Any + Send + Sync + 'static>>,
        handle: WeakHandle,
    }
    impl std::cmp::PartialEq for ZphocTabletModeSwitchV1 {
        #[inline]
        fn eq(&self, other: &ZphocTabletModeSwitchV1) -> bool {
            self.id == other.id
        }
    }
    impl std::cmp::Eq for ZphocTabletModeSwitchV1 {}
    impl PartialEq<Weak<ZphocTabletModeSwitchV1>> for ZphocTabletModeSwitchV1 {
        #[inline]
        fn eq(&self, other: &Weak<ZphocTabletModeSwitchV1>) -> bool {
            self.id == other.id()
        }
    }
    impl std::borrow::Borrow<ObjectId> for ZphocTabletModeSwitchV1 {
        #[inline]
        fn borrow(&self) -> &ObjectId {
            &self.id
        }
    }
    impl std::hash::Hash for ZphocTabletModeSwitchV1 {
        #[inline]
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.id.hash(state)
        }
    }
    impl ::wayland_server::Resource for ZphocTabletModeSwitchV1 {
        type Request = Request;
        type Event<'event> = Event<'event>;
        #[inline]
        fn interface() -> &'static Interface {
            &super::ZPHOC_TABLET_MODE_SWITCH_V1_INTERFACE
        }
        #[inline]
        fn id(&self) -> ObjectId {
            self.id.clone()
        }
        #[inline]
        fn version(&self) -> u32 {
            self.version
        }
        #[inline]
        fn data<U: 'static>(&self) -> Option<&U> {
            self.data
                .as_ref()
                .and_then(|arc| (&**arc).downcast_ref::<ResourceData<Self, U>>())
                .map(|data| &data.udata)
        }
        #[inline]
        fn object_data(&self) -> Option<&Arc<dyn std::any::Any + Send + Sync>> {
            self.data.as_ref()
        }
        fn handle(&self) -> &WeakHandle {
            &self.handle
        }
        #[inline]
        fn from_id(conn: &DisplayHandle, id: ObjectId) -> Result<Self, InvalidId> {
            if !same_interface(id.interface(), Self::interface()) && !id.is_null() {
                return Err(InvalidId);
            }
            let version = conn
                .object_info(id.clone())
                .map(|info| info.version)
                .unwrap_or(0);
            let data = conn.get_object_data(id.clone()).ok();
            Ok(ZphocTabletModeSwitchV1 {
                id,
                data,
                version,
                handle: conn.backend_handle().downgrade(),
            })
        }
        fn send_event(&self, evt: Self::Event<'_>) -> Result<(), InvalidId> {
            let handle = DisplayHandle::from(self.handle.upgrade().ok_or(InvalidId)?);
            handle.send_event(self, evt)
        }
        fn parse_request(
            conn: &DisplayHandle,
            msg: Message<ObjectId, OwnedFd>,
        ) -> Result<(Self, Self::Request), DispatchError> {
            let me = Self::from_id(conn, msg.sender_id.clone()).unwrap();
            let mut arg_iter = msg.args.into_iter();
            match msg.opcode {
                0u16 => {
                    if let () = () {
                        Ok((me, Request::Destroy {}))
                    } else {
                        Err(DispatchError::BadMessage {
                            sender_id: msg.sender_id,
                            interface: Self::interface().name,
                            opcode: msg.opcode,
                        })
                    }
                }
                _ => Err(DispatchError::BadMessage {
                    sender_id: msg.sender_id,
                    interface: Self::interface().name,
                    opcode: msg.opcode,
                }),
            }
        }
        fn write_event<'a>(
            &self,
            conn: &DisplayHandle,
            msg: Self::Event<'a>,
        ) -> Result<Message<ObjectId, std::os::unix::io::BorrowedFd<'a>>, InvalidId> {
            match msg {
                Event::Disabled {} => Ok(Message {
                    sender_id: self.id.clone(),
                    opcode: 0u16,
                    args: smallvec::SmallVec::new(),
                }),
                Event::Enabled {} => Ok(Message {
                    sender_id: self.id.clone(),
                    opcode: 1u16,
                    args: smallvec::SmallVec::new(),
                }),
                Event::__phantom_lifetime { never, .. } => match never {},
            }
        }
        fn __set_object_data(
            &mut self,
            odata: std::sync::Arc<dyn std::any::Any + Send + Sync + 'static>,
        ) {
            self.data = Some(odata);
        }
    }
    impl ZphocTabletModeSwitchV1 {
        #[doc = "Tablet mode got disabled"]
        #[allow(clippy::too_many_arguments)]
        pub fn disabled(&self) {
            let _ = self.send_event(Event::Disabled {});
        }
        #[doc = "Tablet mode got enabled"]
        #[allow(clippy::too_many_arguments)]
        pub fn enabled(&self) {
            let _ = self.send_event(Event::Enabled {});
        }
    }
}
#[doc = "A tablet mode switch\n\nThe lid_switch interface represents a tablet mode switch.\nIt can have two possible values:\n\nThe wl_pointer interface generates enabled and disabled events to indicate\nswitch state changes"]
pub mod zphoc_lid_switch_v1 {
    use ::wayland_server::{
        Dispatch, DispatchError, DisplayHandle, New, Resource, ResourceData, Weak,
        backend::{
            InvalidId, ObjectData, ObjectId, WeakHandle,
            protocol::{Argument, Interface, Message, WEnum, same_interface},
            smallvec,
        },
    };
    use std::os::unix::io::OwnedFd;
    use std::sync::Arc;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this request"]
    pub const REQ_DESTROY_OPCODE: u16 = 0u16;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_OPENED_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this event"]
    pub const EVT_OPENED_OPCODE: u16 = 0u16;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CLOSED_SINCE: u32 = 1u32;
    #[doc = r" The wire opcode for this event"]
    pub const EVT_CLOSED_OPCODE: u16 = 1u16;
    #[non_exhaustive]
    pub enum Request {
        #[doc = "release the switch object\n\nUsing this request a client can tell the server that it is not going to\nuse the switch object anymore.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl Request {
        #[doc = "Get the opcode number of this message"]
        pub fn opcode(&self) -> u16 {
            match *self {
                Request::Destroy => 0u16,
            }
        }
    }
    #[non_exhaustive]
    pub enum Event<'a> {
        #[doc = "Lid got opened"]
        Opened,

        #[doc = "Lid got closed"]
        Closed,

        #[doc(hidden)]
        __phantom_lifetime {
            phantom: std::marker::PhantomData<&'a ()>,
            never: std::convert::Infallible,
        },
    }
    impl<'a> Event<'a> {
        #[doc = "Get the opcode number of this message"]
        pub fn opcode(&self) -> u16 {
            match *self {
                Event::Opened => 0u16,
                Event::Closed => 1u16,
                Event::__phantom_lifetime { never, .. } => match never {},
            }
        }
    }
    #[doc = "A tablet mode switch\n\nThe lid_switch interface represents a tablet mode switch.\nIt can have two possible values:\n\nThe wl_pointer interface generates enabled and disabled events to indicate\nswitch state changes\n\nSee also the [Request] enum for this interface."]
    #[derive(Clone, Debug)]
    pub struct ZphocLidSwitchV1 {
        id: ObjectId,
        version: u32,
        data: Option<Arc<dyn std::any::Any + Send + Sync + 'static>>,
        handle: WeakHandle,
    }
    impl std::cmp::PartialEq for ZphocLidSwitchV1 {
        #[inline]
        fn eq(&self, other: &ZphocLidSwitchV1) -> bool {
            self.id == other.id
        }
    }
    impl std::cmp::Eq for ZphocLidSwitchV1 {}
    impl PartialEq<Weak<ZphocLidSwitchV1>> for ZphocLidSwitchV1 {
        #[inline]
        fn eq(&self, other: &Weak<ZphocLidSwitchV1>) -> bool {
            self.id == other.id()
        }
    }
    impl std::borrow::Borrow<ObjectId> for ZphocLidSwitchV1 {
        #[inline]
        fn borrow(&self) -> &ObjectId {
            &self.id
        }
    }
    impl std::hash::Hash for ZphocLidSwitchV1 {
        #[inline]
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.id.hash(state)
        }
    }
    impl ::wayland_server::Resource for ZphocLidSwitchV1 {
        type Request = Request;
        type Event<'event> = Event<'event>;
        #[inline]
        fn interface() -> &'static Interface {
            &super::ZPHOC_LID_SWITCH_V1_INTERFACE
        }
        #[inline]
        fn id(&self) -> ObjectId {
            self.id.clone()
        }
        #[inline]
        fn version(&self) -> u32 {
            self.version
        }
        #[inline]
        fn data<U: 'static>(&self) -> Option<&U> {
            self.data
                .as_ref()
                .and_then(|arc| (&**arc).downcast_ref::<ResourceData<Self, U>>())
                .map(|data| &data.udata)
        }
        #[inline]
        fn object_data(&self) -> Option<&Arc<dyn std::any::Any + Send + Sync>> {
            self.data.as_ref()
        }
        fn handle(&self) -> &WeakHandle {
            &self.handle
        }
        #[inline]
        fn from_id(conn: &DisplayHandle, id: ObjectId) -> Result<Self, InvalidId> {
            if !same_interface(id.interface(), Self::interface()) && !id.is_null() {
                return Err(InvalidId);
            }
            let version = conn
                .object_info(id.clone())
                .map(|info| info.version)
                .unwrap_or(0);
            let data = conn.get_object_data(id.clone()).ok();
            Ok(ZphocLidSwitchV1 {
                id,
                data,
                version,
                handle: conn.backend_handle().downgrade(),
            })
        }
        fn send_event(&self, evt: Self::Event<'_>) -> Result<(), InvalidId> {
            let handle = DisplayHandle::from(self.handle.upgrade().ok_or(InvalidId)?);
            handle.send_event(self, evt)
        }
        fn parse_request(
            conn: &DisplayHandle,
            msg: Message<ObjectId, OwnedFd>,
        ) -> Result<(Self, Self::Request), DispatchError> {
            let me = Self::from_id(conn, msg.sender_id.clone()).unwrap();
            let mut arg_iter = msg.args.into_iter();
            match msg.opcode {
                0u16 => {
                    if let () = () {
                        Ok((me, Request::Destroy {}))
                    } else {
                        Err(DispatchError::BadMessage {
                            sender_id: msg.sender_id,
                            interface: Self::interface().name,
                            opcode: msg.opcode,
                        })
                    }
                }
                _ => Err(DispatchError::BadMessage {
                    sender_id: msg.sender_id,
                    interface: Self::interface().name,
                    opcode: msg.opcode,
                }),
            }
        }
        fn write_event<'a>(
            &self,
            conn: &DisplayHandle,
            msg: Self::Event<'a>,
        ) -> Result<Message<ObjectId, std::os::unix::io::BorrowedFd<'a>>, InvalidId> {
            match msg {
                Event::Opened {} => Ok(Message {
                    sender_id: self.id.clone(),
                    opcode: 0u16,
                    args: smallvec::SmallVec::new(),
                }),
                Event::Closed {} => Ok(Message {
                    sender_id: self.id.clone(),
                    opcode: 1u16,
                    args: smallvec::SmallVec::new(),
                }),
                Event::__phantom_lifetime { never, .. } => match never {},
            }
        }
        fn __set_object_data(
            &mut self,
            odata: std::sync::Arc<dyn std::any::Any + Send + Sync + 'static>,
        ) {
            self.data = Some(odata);
        }
    }
    impl ZphocLidSwitchV1 {
        #[doc = "Lid got opened"]
        #[allow(clippy::too_many_arguments)]
        pub fn opened(&self) {
            let _ = self.send_event(Event::Opened {});
        }
        #[doc = "Lid got closed"]
        #[allow(clippy::too_many_arguments)]
        pub fn closed(&self) {
            let _ = self.send_event(Event::Closed {});
        }
    }
}
