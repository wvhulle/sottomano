pub mod permissions;
pub mod session_facts;

use std::env;

use iced::futures::{Stream, StreamExt, future::ready};
use zbus::{blocking, proxy, zvariant::OwnedObjectPath};

use crate::message_subscriptions::restarting;

const BOOT_LOADER_MENU_DEFAULT_TIMEOUT: u64 = 0;

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
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
    #[zbus(property)]
    fn docked(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn lid_closed(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn on_external_power(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn n_current_sessions(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn n_current_inhibitors(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn block_inhibited(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn delay_inhibited(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn handle_power_key(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn handle_suspend_key(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn handle_hibernate_key(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn handle_reboot_key(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn handle_lid_switch(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn idle_action(&self) -> zbus::Result<String>;
    #[zbus(property, name = "IdleActionUSec")]
    fn idle_action_usec(&self) -> zbus::Result<u64>;
    #[zbus(property)]
    fn sleep_operation(&self) -> zbus::Result<Vec<String>>;
    #[zbus(property)]
    fn preparing_for_shutdown(&self) -> zbus::Result<bool>;
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

fn manager() -> Option<ManagerProxyBlocking<'static>> {
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
