use std::env;

use iced::futures::{Stream, StreamExt, future::ready, stream::select};
use tokio::process::Child;
use zbus::{blocking, proxy, zvariant::OwnedObjectPath};

use crate::{config::locker::Locker, data::source::command};

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn lock_session(&self, session_id: &str) -> zbus::Result<()>;
    fn get_session(&self, session_id: &str) -> zbus::Result<OwnedObjectPath>;
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1"
)]
trait Session {
    #[zbus(signal)]
    fn lock(&self) -> zbus::Result<()>;
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

pub async fn run_locker(locker: Locker) {
    if locker.command.is_empty() {
        return;
    }
    match watch_lock_requests(locker.lock_before_sleep).await {
        Ok(requests) => spawn_locker_on(requests, &locker.command).await,
        Err(e) => eprintln!("locker: {e}"),
    }
}

async fn watch_lock_requests(lock_before_sleep: bool) -> zbus::Result<impl Stream<Item = ()>> {
    let connection = zbus::Connection::system().await?;
    let manager = ManagerProxy::new(&connection).await?;
    let session_path = manager.get_session(&session_id()).await?;
    let session = SessionProxy::builder(&connection)
        .path(session_path)?
        .build()
        .await?;
    let locks = session.receive_lock().await?.map(|_| ());
    let sleeps = manager
        .receive_prepare_for_sleep()
        .await?
        .filter_map(move |signal| {
            let entering_sleep = signal.args().is_ok_and(|args| *args.start());
            ready((lock_before_sleep && entering_sleep).then_some(()))
        });
    Ok(select(locks, sleeps))
}

async fn spawn_locker_on(requests: impl Stream<Item = ()>, command: &[String]) {
    requests
        .fold(None, |running: Option<Child>, ()| {
            ready(
                running
                    .and_then(still_locked)
                    .or_else(|| start_locker(command)),
            )
        })
        .await;
}

fn still_locked(mut child: Child) -> Option<Child> {
    matches!(child.try_wait(), Ok(None)).then_some(child)
}

fn start_locker(command: &[String]) -> Option<Child> {
    match command::spawn(command) {
        Ok(child) => child,
        Err(e) => {
            eprintln!("locker: {e}");
            None
        }
    }
}
