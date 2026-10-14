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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepEdge {
    Suspending,
    Resumed,
}

pub fn sleep_edges() -> impl Stream<Item = SleepEdge> + Send {
    restarting("sleep signal", sleep_signals)
}

pub fn resumes() -> impl Stream<Item = ()> {
    sleep_edges().filter_map(|edge| ready((edge == SleepEdge::Resumed).then_some(())))
}

async fn sleep_signals() -> zbus::Result<impl Stream<Item = SleepEdge> + Send> {
    let connection = zbus::Connection::system().await?;
    let manager = ManagerProxy::new(&connection).await?;
    let edges = manager
        .receive_prepare_for_sleep()
        .await?
        .filter_map(|signal| {
            let edge = signal.args().ok().map(|args| {
                if *args.start() {
                    SleepEdge::Suspending
                } else {
                    SleepEdge::Resumed
                }
            });
            ready(edge)
        });
    Ok(edges)
}
