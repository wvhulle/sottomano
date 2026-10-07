use std::env;

use iced::futures::{Stream, StreamExt, future::ready};
use zbus::{blocking, proxy};

use crate::message_subscriptions::restarting;

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn lock_session(&self, session_id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

fn manager() -> Option<ManagerProxyBlocking<'static>> {
    ManagerProxyBlocking::new(&blocking::Connection::system().ok()?).ok()
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

fn session_id() -> String {
    env::var("XDG_SESSION_ID").unwrap_or_else(|_| "auto".to_owned())
}

pub fn lock() {
    if let Some(proxy) = manager() {
        let _ = proxy.lock_session(&session_id());
    }
}

pub fn resumes() -> impl Stream<Item = ()> {
    restarting("resume signal", resume_signals)
}

async fn resume_signals() -> zbus::Result<impl Stream<Item = ()>> {
    let connection = zbus::Connection::system().await?;
    let manager = ManagerProxy::new(&connection).await?;
    let resumes = manager
        .receive_prepare_for_sleep()
        .await?
        .filter_map(|signal| {
            let resumed = signal.args().is_ok_and(|args| !*args.start());
            ready(resumed.then_some(()))
        });
    Ok(resumes)
}
