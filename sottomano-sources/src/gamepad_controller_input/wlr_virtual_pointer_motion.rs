use std::{
    fmt, io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use sottomano_config::widget_groups::gamepad_bindings::PointerButton;
use wayrs_client::{Connection, IoMode, global::BindError, protocol::wl_pointer::ButtonState};
use wayrs_protocols::wlr_virtual_pointer_unstable_v1::{
    ZwlrVirtualPointerManagerV1, ZwlrVirtualPointerV1,
};

use super::stick_motion_tracking::PointerDisplacement;

#[derive(Debug, Clone)]
pub struct PointerGate(Arc<AtomicBool>);

impl Default for PointerGate {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
}

impl PartialEq for PointerGate {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for PointerGate {}

impl PointerGate {
    pub fn allow(&self, allowed: bool) {
        self.0.store(allowed, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

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
    pub fn connect() -> Result<Self, VirtualPointerError> {
        let mut conn = Connection::<()>::connect().map_err(VirtualPointerError::Connect)?;
        conn.blocking_roundtrip().map_err(VirtualPointerError::Io)?;
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

    fn timestamp(&self) -> u32 {
        self.created.elapsed().as_millis() as u32
    }

    pub fn move_by(&mut self, displacement: PointerDisplacement) -> io::Result<()> {
        let time = self.timestamp();
        self.pointer.motion(
            &mut self.conn,
            time,
            displacement.dx.into(),
            displacement.dy.into(),
        );
        self.pointer.frame(&mut self.conn);
        self.conn.flush(IoMode::Blocking)
    }

    pub fn set_button(&mut self, button: PointerButton, pressed: bool) -> io::Result<()> {
        let time = self.timestamp();
        let state = if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        self.pointer
            .button(&mut self.conn, time, evdev_button_code(button), state);
        self.pointer.frame(&mut self.conn);
        self.conn.flush(IoMode::Blocking)
    }
}

const fn evdev_button_code(button: PointerButton) -> u32 {
    match button {
        PointerButton::Left => 0x110,
        PointerButton::Right => 0x111,
        PointerButton::Middle => 0x112,
    }
}
