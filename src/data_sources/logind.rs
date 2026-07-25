//! One-shot power/lock actions via systemd-logind over the **system** bus
//! (`org.freedesktop.login1`) — the manual counterpart to the idle daemon's
//! timed actions. Blocking and fire-and-forget, modeled on [`mpris::control`];
//! call off the render path. `interactive = true` so `PolicyKit` may prompt and
//! inhibitor delays are honored.
//!
//! [`mpris::control`]: crate::data_sources::mpris::control

use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::DynamicType;

const DEST: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";

fn call<A: serde::Serialize + DynamicType>(method: &str, args: &A) {
    let Ok(connection) = Connection::system() else {
        return;
    };
    if let Ok(proxy) = Proxy::new(&connection, DEST, PATH, MANAGER) {
        let _: zbus::Result<()> = proxy.call(method, args);
    }
}

pub fn suspend() {
    call("Suspend", &(true,));
}

pub fn power_off() {
    call("PowerOff", &(true,));
}

/// Lock the caller's session (equivalent to `loginctl lock-session`). Only takes
/// visible effect if a lock handler is listening for logind's `Lock` signal.
pub fn lock() {
    let Ok(session) = std::env::var("XDG_SESSION_ID") else {
        return;
    };
    call("LockSession", &(session,));
}
