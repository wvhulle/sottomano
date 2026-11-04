use std::env;

use futures::{Stream, StreamExt, future::ready};
use serde::Deserialize;
use zbus::{
    blocking, proxy,
    zvariant::{OwnedObjectPath, Type},
};

use crate::{
    stream_operators::connected_until_failure,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

const BOOT_LOADER_MENU_DEFAULT_TIMEOUT: u64 = 0;

#[derive(Debug, Deserialize, Type)]
#[allow(dead_code)]
pub struct InhibitorRow {
    pub what: String,
    pub who: String,
    pub why: String,
    pub mode: String,
    uid: u32,
    pub pid: u32,
}

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub(super) trait Manager {
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
    fn hibernate(&self, interactive: bool) -> zbus::Result<()>;
    fn hybrid_sleep(&self, interactive: bool) -> zbus::Result<()>;
    fn suspend_then_hibernate(&self, interactive: bool) -> zbus::Result<()>;
    fn sleep(&self, flags: u64) -> zbus::Result<()>;
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
    fn halt(&self, interactive: bool) -> zbus::Result<()>;
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn set_reboot_to_firmware_setup(&self, enable: bool) -> zbus::Result<()>;
    fn set_reboot_to_boot_loader_menu(&self, timeout: u64) -> zbus::Result<()>;
    fn cancel_scheduled_shutdown(&self) -> zbus::Result<bool>;
    fn lock_session(&self, session_id: &str) -> zbus::Result<()>;
    fn lock_sessions(&self) -> zbus::Result<()>;
    fn terminate_session(&self, session_id: &str) -> zbus::Result<()>;
    fn get_session(&self, session_id: &str) -> zbus::Result<OwnedObjectPath>;
    fn can_sleep(&self) -> zbus::Result<String>;
    fn can_suspend(&self) -> zbus::Result<String>;
    fn can_hibernate(&self) -> zbus::Result<String>;
    fn can_hybrid_sleep(&self) -> zbus::Result<String>;
    fn can_suspend_then_hibernate(&self) -> zbus::Result<String>;
    fn can_reboot(&self) -> zbus::Result<String>;
    fn can_reboot_to_firmware_setup(&self) -> zbus::Result<String>;
    fn can_reboot_to_boot_loader_menu(&self) -> zbus::Result<String>;
    fn can_halt(&self) -> zbus::Result<String>;
    fn can_power_off(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn scheduled_shutdown(&self) -> zbus::Result<(String, u64)>;
    fn list_inhibitors(&self) -> zbus::Result<Vec<InhibitorRow>>;
    #[zbus(property)]
    fn idle_action(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn sleep_operation(&self) -> zbus::Result<Vec<String>>;
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

pub(super) fn manager() -> Option<ManagerProxyBlocking<'static>> {
    ManagerProxyBlocking::new(&blocking::Connection::system().ok()?).ok()
}

fn with_manager(call: impl FnOnce(&ManagerProxyBlocking<'static>) -> zbus::Result<()>) {
    if let Some(proxy) = manager()
        && let Err(e) = call(&proxy)
    {
        tracing::warn!("logind: {e}");
    }
}

pub fn suspend() {
    with_manager(|proxy| proxy.suspend(true));
}

pub fn hibernate() {
    with_manager(|proxy| proxy.hibernate(true));
}

pub fn hybrid_sleep() {
    with_manager(|proxy| proxy.hybrid_sleep(true));
}

pub fn suspend_then_hibernate() {
    with_manager(|proxy| proxy.suspend_then_hibernate(true));
}

pub fn sleep() {
    with_manager(|proxy| proxy.sleep(0));
}

pub fn reboot() {
    with_manager(|proxy| proxy.reboot(true));
}

pub fn reboot_to_firmware_setup() {
    with_manager(|proxy| {
        proxy.set_reboot_to_firmware_setup(true)?;
        proxy.reboot(true)
    });
}

pub fn reboot_to_boot_loader_menu() {
    with_manager(|proxy| {
        proxy.set_reboot_to_boot_loader_menu(BOOT_LOADER_MENU_DEFAULT_TIMEOUT)?;
        proxy.reboot(true)
    });
}

pub fn halt() {
    with_manager(|proxy| proxy.halt(true));
}

pub fn power_off() {
    with_manager(|proxy| proxy.power_off(true));
}

pub fn cancel_scheduled_shutdown() {
    with_manager(|proxy| proxy.cancel_scheduled_shutdown().map(|_| ()));
}

#[must_use]
pub fn session_id() -> String {
    env::var("XDG_SESSION_ID").unwrap_or_else(|_| "auto".to_owned())
}

pub fn lock() {
    with_manager(|proxy| proxy.lock_session(&session_id()));
}

pub fn lock_all_sessions() {
    with_manager(ManagerProxyBlocking::lock_sessions);
}

pub fn terminate_session() {
    with_manager(|proxy| proxy.terminate_session(&session_id()));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepEdge {
    Suspending,
    Resumed,
}

pub fn sleep_edges(buses: &DesktopBuses) -> impl Stream<Item = SleepEdge> + Send + use<> {
    connected_until_failure(sleep_signals(buses.system.clone())).filter_map(|edge| {
        ready(
            edge.inspect_err(|e| tracing::warn!("logind sleep signal: {e}"))
                .ok(),
        )
    })
}

pub fn resumes(buses: &DesktopBuses) -> impl Stream<Item = ()> + Send + use<> {
    sleep_edges(buses).filter_map(|edge| ready((edge == SleepEdge::Resumed).then_some(())))
}

async fn sleep_signals(
    system: zbus::Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<SleepEdge>> + Send> {
    let manager = ManagerProxy::new(&system).await?;
    let edges = manager.receive_prepare_for_sleep().await?.map(|signal| {
        let args = signal.args()?;
        Ok(if *args.start() {
            SleepEdge::Suspending
        } else {
            SleepEdge::Resumed
        })
    });
    Ok(edges)
}
