use std::{fmt, io, time::Instant};

use sottomano_toml_config::widget_groups::gamepad_bindings::PointerButton;
use wayrs_client::{Connection, global::BindError, protocol::wl_pointer::ButtonState};
use wayrs_protocols::wlr_virtual_pointer_unstable_v1::{
    ZwlrVirtualPointerManagerV1, ZwlrVirtualPointerV1,
};

use super::gamepad_input_machine::PointerEffect;

#[derive(Debug)]
pub enum VirtualPointerError {
    Connect(wayrs_client::ConnectError),
    Unsupported(BindError),
    Io(io::Error),
}

impl fmt::Display for VirtualPointerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(e) => write!(f, "could not connect to wayland: {e}"),
            Self::Unsupported(e) => write!(f, "compositor offers no virtual pointer: {e}"),
            Self::Io(e) => write!(f, "wayland io: {e}"),
        }
    }
}

pub struct VirtualPointer {
    conn: Connection<()>,
    pointer: ZwlrVirtualPointerV1,
    created: Instant,
}

impl VirtualPointer {
    pub async fn connect() -> Result<Self, VirtualPointerError> {
        let mut conn = Connection::<()>::connect().map_err(VirtualPointerError::Connect)?;
        conn.async_roundtrip()
            .await
            .map_err(VirtualPointerError::Io)?;
        let manager: ZwlrVirtualPointerManagerV1 = conn
            .bind_singleton(1..=2)
            .map_err(VirtualPointerError::Unsupported)?;
        let pointer = manager.create_virtual_pointer(&mut conn, None);
        Ok(Self {
            conn,
            pointer,
            created: Instant::now(),
        })
    }

    pub async fn apply(&mut self, effect: PointerEffect) -> io::Result<()> {
        let time = self.created.elapsed().as_millis() as u32;
        match effect {
            PointerEffect::Move(displacement) => self.pointer.motion(
                &mut self.conn,
                time,
                displacement.dx.into(),
                displacement.dy.into(),
            ),
            PointerEffect::Button { button, pressed } => self.pointer.button(
                &mut self.conn,
                time,
                evdev_button_code(button),
                if pressed {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                },
            ),
        }
        self.pointer.frame(&mut self.conn);
        self.conn.async_flush().await
    }
}

const fn evdev_button_code(button: PointerButton) -> u32 {
    match button {
        PointerButton::Left => 0x110,
        PointerButton::Right => 0x111,
        PointerButton::Middle => 0x112,
    }
}
