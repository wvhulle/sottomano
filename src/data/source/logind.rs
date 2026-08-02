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

pub fn lock() {
    let Ok(session) = std::env::var("XDG_SESSION_ID") else {
        return;
    };
    if let Some(proxy) = manager() {
        let _ = proxy.lock_session(&session);
    }
}
