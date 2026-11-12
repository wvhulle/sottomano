use std::{
    collections::BTreeMap,
    convert::Infallible,
    future::ready,
    path::{Path, PathBuf},
    time::Duration,
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt};
use serde::Deserialize;

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::{Availability, AvailabilityStreams},
    delivery_cadence_drivers::PeriodicPolling,
    podman_container_engine::libpod_running_containers::{
        EngineReadError, classified_engine_failure,
    },
    system_protocol_clients::{http_json_client, inotify_file_presence::path_present},
};

pub const INCUS_API_ROOT: &str = "http://incus/1.0";
const INSTANCE_POLL_PERIOD: Duration = Duration::from_secs(5);
const RUNNING_STATUS: &str = "Running";
const GLOBAL_ADDRESS_SCOPE: &str = "global";
const LOOPBACK_INTERFACE: &str = "lo";

impl SnapshotSource for Availability {
    type Error = Infallible;
    type StreamInputs = InstanceDaemon;

    const NAME: &'static str = "Incus daemon presence";

    fn delivery_stream(
        instance_daemon: InstanceDaemon,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(instance_daemon, |instance_daemon| {
            ready(Ok(path_present(instance_daemon.socket_path()).map(Ok)))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonInstanceListing {
    pub instance_name: String,
    pub project_name: String,
    pub kind: InstanceKind,
    pub running: bool,
    pub status_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DaemonInstanceListings {
    Instances(Vec<DaemonInstanceListing>),
    Unreadable(EngineReadError),
}

async fn read_daemon_instance_listings(socket_path: &Path) -> DaemonInstanceListings {
    let listing: Result<InstanceListingReply, _> = http_json_client::get_json_over_unix_socket(
        socket_path,
        &format!("{INCUS_API_ROOT}/instances?recursion=1&all-projects=true"),
    )
    .await;
    match listing {
        Ok(listing) => {
            let mut listings: Vec<DaemonInstanceListing> = listing
                .metadata
                .into_iter()
                .map(|listed| DaemonInstanceListing {
                    instance_name: listed.name,
                    project_name: listed.project,
                    kind: instance_kind_of(&listed.instance_type),
                    running: listed.status == RUNNING_STATUS,
                    status_text: listed.status,
                })
                .collect();
            listings.sort_by(|first, second| first.instance_name.cmp(&second.instance_name));
            DaemonInstanceListings::Instances(listings)
        }
        Err(fetch_failure) => DaemonInstanceListings::Unreadable(
            classified_engine_failure(socket_path, fetch_failure).await,
        ),
    }
}

impl SnapshotSource for DaemonInstanceListings {
    type Error = Infallible;
    type StreamInputs = InstanceDaemon;

    const NAME: &'static str = "Incus instance names";

    fn delivery_stream(
        instance_daemon: InstanceDaemon,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let watched_daemon = instance_daemon;
        path_present(instance_daemon.socket_path()).while_available(
            move || {
                let watched_daemon = watched_daemon;
                PeriodicPolling::every(INSTANCE_POLL_PERIOD).poll_deliveries(move || async move {
                    Ok(read_daemon_instance_listings(&watched_daemon.socket_path()).await)
                })
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

fn instance_kind_of(instance_type: &str) -> InstanceKind {
    if instance_type == "virtual-machine" {
        InstanceKind::VirtualMachine
    } else {
        InstanceKind::Container
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstanceDaemon {
    Incus,
    Lxd,
}

impl InstanceDaemon {
    #[must_use]
    pub const fn product_name(self) -> &'static str {
        match self {
            InstanceDaemon::Incus => "Incus",
            InstanceDaemon::Lxd => "LXD",
        }
    }

    #[must_use]
    pub fn socket_path(self) -> PathBuf {
        PathBuf::from(match self {
            InstanceDaemon::Incus => "/var/lib/incus/unix.socket",
            InstanceDaemon::Lxd => "/var/lib/lxd/unix.socket",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceKind {
    Container,
    VirtualMachine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonInstance {
    pub instance_name: String,
    pub project_name: String,
    pub kind: InstanceKind,
    pub running: bool,
    pub status_text: String,
    pub memory_in_use: Option<ByteSize>,
    pub global_addresses: Vec<String>,
    pub image_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DaemonInstancesReadout {
    Instances(Vec<DaemonInstance>),
    Unreadable(EngineReadError),
}

#[derive(Deserialize)]
struct InstanceAddress {
    #[serde(default)]
    address: String,
    #[serde(default)]
    scope: String,
}

#[derive(Deserialize)]
struct InstanceNetworkInterface {
    #[serde(default)]
    addresses: Vec<InstanceAddress>,
}

#[derive(Deserialize)]
struct InstanceMemoryState {
    #[serde(default)]
    usage: u64,
}

#[derive(Deserialize)]
struct InstanceRuntimeState {
    memory: Option<InstanceMemoryState>,
    network: Option<BTreeMap<String, InstanceNetworkInterface>>,
}

#[derive(Deserialize)]
struct ListedInstance {
    name: String,
    #[serde(default)]
    project: String,
    #[serde(rename = "type", default)]
    instance_type: String,
    #[serde(default)]
    status: String,
    state: Option<InstanceRuntimeState>,
    #[serde(default)]
    config: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct InstanceListingReply {
    metadata: Vec<ListedInstance>,
}

impl From<ListedInstance> for DaemonInstance {
    fn from(listed: ListedInstance) -> Self {
        let running = listed.status == RUNNING_STATUS;
        let runtime_state = listed.state.filter(|_| running);
        let global_addresses = runtime_state
            .as_ref()
            .and_then(|runtime_state| runtime_state.network.as_ref())
            .into_iter()
            .flat_map(|interfaces| interfaces.iter())
            .filter(|(interface_name, _)| interface_name.as_str() != LOOPBACK_INTERFACE)
            .flat_map(|(_, interface)| interface.addresses.iter())
            .filter(|interface_address| interface_address.scope == GLOBAL_ADDRESS_SCOPE)
            .map(|interface_address| interface_address.address.clone())
            .collect();
        DaemonInstance {
            instance_name: listed.name,
            project_name: listed.project,
            kind: instance_kind_of(&listed.instance_type),
            running,
            status_text: listed.status,
            memory_in_use: runtime_state
                .and_then(|runtime_state| runtime_state.memory)
                .map(|memory_state| ByteSize::b(memory_state.usage))
                .filter(|usage| usage.as_u64() > 0),
            global_addresses,
            image_description: listed.config.get("image.description").cloned(),
        }
    }
}

async fn read_daemon_instances(socket_path: &Path) -> DaemonInstancesReadout {
    let listing: Result<InstanceListingReply, _> = http_json_client::get_json_over_unix_socket(
        socket_path,
        &format!("{INCUS_API_ROOT}/instances?recursion=2&all-projects=true"),
    )
    .await;
    match listing {
        Ok(listing) => {
            let mut instances: Vec<DaemonInstance> = listing
                .metadata
                .into_iter()
                .map(DaemonInstance::from)
                .collect();
            instances.sort_by(|first, second| first.instance_name.cmp(&second.instance_name));
            DaemonInstancesReadout::Instances(instances)
        }
        Err(fetch_failure) => DaemonInstancesReadout::Unreadable(
            classified_engine_failure(socket_path, fetch_failure).await,
        ),
    }
}

impl SnapshotSource for DaemonInstancesReadout {
    type Error = Infallible;
    type StreamInputs = InstanceDaemon;

    const NAME: &'static str = "Incus instances";

    fn delivery_stream(
        instance_daemon: InstanceDaemon,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let watched_daemon = instance_daemon;
        path_present(instance_daemon.socket_path()).while_available(
            move || {
                let watched_daemon = watched_daemon;
                PeriodicPolling::every(INSTANCE_POLL_PERIOD).poll_deliveries(move || async move {
                    Ok(read_daemon_instances(&watched_daemon.socket_path()).await)
                })
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;
    use serde_json::json;

    use super::{DaemonInstance, InstanceKind, InstanceListingReply};

    #[test]
    fn a_running_container_keeps_its_global_addresses_and_memory() {
        let listing: InstanceListingReply = serde_json::from_value(json!({
            "metadata": [{
                "name": "build-box",
                "project": "default",
                "type": "container",
                "status": "Running",
                "config": {"image.description": "Debian trixie amd64"},
                "state": {
                    "memory": {"usage": 268_435_456},
                    "network": {
                        "lo": {"addresses": [{"address": "127.0.0.1", "scope": "local"}]},
                        "eth0": {"addresses": [
                            {"address": "10.41.7.23", "scope": "global"},
                            {"address": "fe80::1", "scope": "link"}
                        ]}
                    }
                }
            }, {
                "name": "win-vm",
                "project": "default",
                "type": "virtual-machine",
                "status": "Stopped",
                "config": {},
                "state": null
            }]
        }))
        .unwrap();
        let instances: Vec<DaemonInstance> = listing
            .metadata
            .into_iter()
            .map(DaemonInstance::from)
            .collect();
        assert_eq!(instances[0].global_addresses, ["10.41.7.23"]);
        assert_eq!(instances[0].memory_in_use, Some(ByteSize::mib(256)));
        assert_eq!(
            instances[0].image_description.as_deref(),
            Some("Debian trixie amd64")
        );
        assert_eq!(instances[1].kind, InstanceKind::VirtualMachine);
        assert!(!instances[1].running);
        assert_eq!(instances[1].memory_in_use, None);
    }
}
