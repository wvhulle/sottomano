use std::{
    collections::HashMap,
    convert::Infallible,
    future::ready,
    path::{Path, PathBuf},
    time::Duration,
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt};
use zbus::proxy;

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::Availability,
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses,
        inotify_file_presence::path_present,
        sysfs_cgroup_memory::{CGROUP_ROOT, read_cgroup_memory},
    },
};

const SESSION_POLL_PERIOD: Duration = Duration::from_secs(5);
const WAYDROID_PAYLOAD_CGROUP: &str = "lxc.payload.waydroid";

#[proxy(
    interface = "id.waydro.ContainerManager",
    default_service = "id.waydro.Container",
    default_path = "/ContainerManager",
    gen_blocking = false
)]
pub trait WaydroidContainerManager {
    fn get_session(&self) -> zbus::Result<HashMap<String, String>>;

    fn stop(&self, quit_session: bool) -> zbus::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaydroidState {
    Absent,
    ContainerOnly,
    Session {
        session_state: String,
        user_name: Option<String>,
        lcd_density: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaydroidProbe(pub Availability);

impl SnapshotSource for WaydroidProbe {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "Waydroid presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries((), |()| {
            ready(Ok(
                path_present(waydroid_payload_cgroup()).map(|presence| Ok(WaydroidProbe(presence)))
            ))
        })
    }
}

fn waydroid_payload_cgroup() -> PathBuf {
    Path::new(CGROUP_ROOT).join(WAYDROID_PAYLOAD_CGROUP)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaydroidListing {
    pub state: WaydroidState,
    pub container_running: bool,
}

impl SnapshotSource for WaydroidListing {
    type Error = Infallible;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Waydroid state";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(SESSION_POLL_PERIOD).poll_deliveries(move || {
            let system_bus = buses.system.clone();
            async move { Ok(read_waydroid_listing(&system_bus).await) }
        })
    }
}

async fn waydroid_session(system_bus: &zbus::Connection) -> HashMap<String, String> {
    let session = match WaydroidContainerManagerProxy::new(system_bus).await {
        Ok(manager) => manager.get_session().await,
        Err(proxy_failure) => Err(proxy_failure),
    };
    session.unwrap_or_else(|session_failure| {
        tracing::debug!("waydroid: no session: {session_failure}");
        HashMap::new()
    })
}

async fn read_waydroid_listing(system_bus: &zbus::Connection) -> WaydroidListing {
    let container_running = waydroid_payload_cgroup().exists();
    let session = waydroid_session(system_bus).await;
    WaydroidListing {
        state: state_of_session(&session, container_running),
        container_running,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaydroidReadout {
    pub state: WaydroidState,
    pub container_running: bool,
    pub memory_in_use: Option<ByteSize>,
}

fn state_of_session(session: &HashMap<String, String>, container_running: bool) -> WaydroidState {
    if session.is_empty() {
        return if container_running {
            WaydroidState::ContainerOnly
        } else {
            WaydroidState::Absent
        };
    }
    WaydroidState::Session {
        session_state: session
            .get("state")
            .cloned()
            .unwrap_or_else(|| "RUNNING".to_owned()),
        user_name: session.get("user_name").cloned(),
        lcd_density: session.get("lcd_density").cloned(),
    }
}

async fn read_waydroid(system_bus: &zbus::Connection) -> WaydroidReadout {
    let payload_cgroup = waydroid_payload_cgroup();
    let container_memory = read_cgroup_memory(&payload_cgroup);
    let container_running = container_memory.is_some();
    let session = waydroid_session(system_bus).await;
    WaydroidReadout {
        state: state_of_session(&session, container_running),
        container_running,
        memory_in_use: container_memory.map(|reading| reading.current),
    }
}

impl SnapshotSource for WaydroidReadout {
    type Error = Infallible;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Waydroid";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(SESSION_POLL_PERIOD).poll_deliveries(move || {
            let system_bus = buses.system.clone();
            async move { Ok(read_waydroid(&system_bus).await) }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{WaydroidState, state_of_session};

    #[test]
    fn no_session_and_no_container_means_waydroid_is_absent() {
        assert_eq!(
            state_of_session(&HashMap::new(), false),
            WaydroidState::Absent
        );
        assert_eq!(
            state_of_session(&HashMap::new(), true),
            WaydroidState::ContainerOnly
        );
    }

    #[test]
    fn a_session_reports_its_state_and_user() {
        let session = HashMap::from([
            ("state".to_owned(), "FROZEN".to_owned()),
            ("user_name".to_owned(), "wvhulle".to_owned()),
        ]);
        assert_eq!(
            state_of_session(&session, true),
            WaydroidState::Session {
                session_state: "FROZEN".to_owned(),
                user_name: Some("wvhulle".to_owned()),
                lcd_density: None,
            }
        );
    }
}
