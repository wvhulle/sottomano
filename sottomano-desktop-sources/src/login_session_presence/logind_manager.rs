use std::env;

use futures::{Stream, StreamExt, future::ready, stream};
use futures_rx::RxExt;
use serde::Deserialize;
use zbus::{
    proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, Type},
};

use crate::system_protocol_clients::{
    dbus_desktop_buses::DesktopBuses, protocol_peer_reconnects::subscribed,
};

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
    default_path = "/org/freedesktop/login1",
    gen_blocking = false
)]
pub trait Manager {
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
    #[zbus(property)]
    fn preparing_for_sleep(&self) -> zbus::Result<bool>;
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

#[must_use]
pub fn session_id() -> String {
    env::var("XDG_SESSION_ID").unwrap_or_else(|_| "auto".to_owned())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepEdge {
    Suspending,
    Resumed,
}

impl SleepEdge {
    fn while_preparing_for_sleep(preparing_for_sleep: bool) -> Self {
        if preparing_for_sleep {
            SleepEdge::Suspending
        } else {
            SleepEdge::Resumed
        }
    }
}

pub fn sleep_edges(buses: &DesktopBuses) -> impl Stream<Item = SleepEdge> + Send + use<> {
    subscribed(buses.system.clone(), sleep_signals)
        .filter_map(|edge| {
            ready(
                edge.inspect_err(|sleep_failure| {
                    tracing::warn!("logind sleep signal: {sleep_failure}");
                })
                .ok(),
            )
        })
        .distinct_until_changed()
}

pub fn resumes(buses: &DesktopBuses) -> impl Stream<Item = ()> + Send + use<> {
    sleep_edges(buses).filter_map(|edge| ready((edge == SleepEdge::Resumed).then_some(())))
}

async fn sleep_signals(
    system: zbus::Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<SleepEdge>> + Send> {
    let manager = ManagerProxy::builder(&system)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let signalled_edges = manager.receive_prepare_for_sleep().await?.map(|signal| {
        Ok(SleepEdge::while_preparing_for_sleep(
            *signal.args()?.start(),
        ))
    });
    let edge_at_subscribe =
        SleepEdge::while_preparing_for_sleep(manager.preparing_for_sleep().await?);
    Ok(stream::iter([Ok(edge_at_subscribe)]).chain(signalled_edges))
}
