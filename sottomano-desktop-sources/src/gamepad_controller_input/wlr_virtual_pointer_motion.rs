use std::time::Instant;

use sottomano_toml_config::indicator_groups::gamepad_bindings::PointerButton;
use wayland_client::{
    Connection, Dispatch, QueueHandle, delegate_noop,
    globals::GlobalListContents,
    protocol::{
        wl_pointer::ButtonState,
        wl_registry::{self, WlRegistry},
    },
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

use super::{evdev_gamepad_devices::ButtonEdge, gamepad_input_machine::PointerEffect};
use crate::system_protocol_clients::wayland_async_connection::{
    AsyncWaylandConnection, WaylandProtocolError,
};

pub struct VirtualPointerEvents;

impl Dispatch<WlRegistry, GlobalListContents> for VirtualPointerEvents {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(VirtualPointerEvents: ZwlrVirtualPointerManagerV1);
delegate_noop!(VirtualPointerEvents: ZwlrVirtualPointerV1);

pub struct VirtualPointer {
    wayland_connection: AsyncWaylandConnection<VirtualPointerEvents>,
    pointer: ZwlrVirtualPointerV1,
    created: Instant,
}

impl VirtualPointer {
    pub async fn connect() -> Result<Self, WaylandProtocolError> {
        let wayland_connection = AsyncWaylandConnection::<VirtualPointerEvents>::connect().await?;
        let manager: ZwlrVirtualPointerManagerV1 = wayland_connection
            .globals
            .bind(&wayland_connection.queue_handle, 1..=2, ())
            .map_err(|source| WaylandProtocolError::ProtocolUnavailable {
                protocol: "zwlr_virtual_pointer_manager_v1",
                source,
            })?;
        let pointer = manager.create_virtual_pointer(None, &wayland_connection.queue_handle, ());
        Ok(Self {
            wayland_connection,
            pointer,
            created: Instant::now(),
        })
    }

    pub async fn apply(&mut self, effect: PointerEffect) -> Result<(), WaylandProtocolError> {
        let time = Instant::now()
            .checked_duration_since(self.created)
            .expect("the virtual pointer exists before it moves")
            .as_millis() as u32;
        match effect {
            PointerEffect::Move(displacement) => {
                self.pointer.motion(time, displacement.dx, displacement.dy);
            }
            PointerEffect::Button { button, edge } => self.pointer.button(
                time,
                evdev_button_code(button),
                match edge {
                    ButtonEdge::Pressed => ButtonState::Pressed,
                    ButtonEdge::Released => ButtonState::Released,
                },
            ),
        }
        self.pointer.frame();
        self.wayland_connection.flushed().await
    }
}

const fn evdev_button_code(button: PointerButton) -> u32 {
    match button {
        PointerButton::Left => 0x110,
        PointerButton::Right => 0x111,
        PointerButton::Middle => 0x112,
    }
}
