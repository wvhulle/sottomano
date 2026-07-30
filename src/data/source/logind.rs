//! One-shot power/lock actions via systemd-logind over the **system** bus
//! (`org.freedesktop.login1`) — the manual counterpart to the idle daemon's
//! timed actions. Blocking and fire-and-forget, modeled on [`mpris::control`];
//! call off the render path. `interactive = true` so `PolicyKit` may prompt and
//! inhibitor delays are honored.
//!
//! [`mpris::control`]: crate::data::source::mpris::control

use zbus::{blocking::Connection, proxy};

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn lock_session(&self, session_id: &str) -> zbus::Result<()>;
}

fn manager() -> Option<ManagerProxyBlocking<'static>> {
    ManagerProxyBlocking::new(&Connection::system().ok()?).ok()
}

pub fn suspend() {
    if let Some(proxy) = manager() {
        let _ = proxy.suspend(true);
    }
}

pub fn power_off() {
    if let Some(proxy) = manager() {
        let _ = proxy.power_off(true);
    }
}

/// Lock the caller's session (equivalent to `loginctl lock-session`). Only
/// takes visible effect if a lock handler is listening for logind's `Lock`
/// signal.
pub fn lock() {
    let Ok(session) = std::env::var("XDG_SESSION_ID") else {
        return;
    };
    if let Some(proxy) = manager() {
        let _ = proxy.lock_session(&session);
    }
}
