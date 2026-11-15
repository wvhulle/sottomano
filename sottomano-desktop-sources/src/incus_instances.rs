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
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_run_state::ContainerRunState,
    delivery_cadence_drivers::PeriodicPolling,
    podman_container_engine::libpod_running_containers::{
        EngineReadError, classified_engine_failure, unavailable_when_socket_missing,
    },
    system_protocol_clients::{http_json_client, inotify_file_presence::path_present},
};

pub const INCUS_API_ROOT: &str = "http://incus/1.0";
const INSTANCE_POLL_PERIOD: Duration = Duration::from_secs(5);
const RUNNING_STATUS: &str = "Running";
const GLOBAL_ADDRESS_SCOPE: &str = "global";
const LOOPBACK_INTERFACE: &str = "lo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceDaemonProbe(pub Availability);

impl SnapshotSource for InstanceDaemonProbe {
    type Error = Infallible;
    type StreamInputs = InstanceDaemon;

    const NAME: &'static str = "Incus daemon presence";

    fn delivery_stream(
        instance_daemon: InstanceDaemon,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(instance_daemon, |instance_daemon| {
            ready(Ok(path_present(instance_daemon.socket_path())
                .map(|presence| Ok(InstanceDaemonProbe(presence)))))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonInstanceListing {
    pub instance_name: String,
    pub project_name: String,
    pub kind: InstanceKind,
    pub run_state: ContainerRunState,
    pub status_text: String,
}

async fn read_daemon_instance_listings(
    socket_path: &Path,
) -> Result<FamilyListings<DaemonInstanceListing>, EngineReadError> {
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
                .map(|listed| DaemonInstanceListing::from(&listed))
                .collect();
            listings.sort_by(|first, second| first.instance_name.cmp(&second.instance_name));
            Ok(FamilyListings { listings })
        }
        Err(fetch_failure) => Err(classified_engine_failure(socket_path, fetch_failure).await),
    }
}

impl SnapshotSource for FamilyListings<DaemonInstanceListing> {
    type Error = EngineReadError;
    type StreamInputs = InstanceDaemon;

    const NAME: &'static str = "Incus instance names";

    fn delivery_stream(
        instance_daemon: InstanceDaemon,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let watched_daemon = instance_daemon;
        path_present(instance_daemon.socket_path()).while_available(
            move || {
                let watched_daemon = watched_daemon;
                PeriodicPolling::every(INSTANCE_POLL_PERIOD)
                    .poll_deliveries(move || async move {
                        read_daemon_instance_listings(&watched_daemon.socket_path()).await
                    })
                    .map(unavailable_when_socket_missing)
            },
            || SnapshotDelivery::SourceUnavailable,
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
    pub listing: DaemonInstanceListing,
    pub memory_in_use: Option<ByteSize>,
    pub global_addresses: Vec<String>,
    pub image_description: Option<String>,
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

impl From<&ListedInstance> for DaemonInstanceListing {
    fn from(listed: &ListedInstance) -> Self {
        DaemonInstanceListing {
            instance_name: listed.name.clone(),
            project_name: listed.project.clone(),
            kind: instance_kind_of(&listed.instance_type),
            run_state: if listed.status == RUNNING_STATUS {
                ContainerRunState::Running
            } else {
                ContainerRunState::Stopped
            },
            status_text: listed.status.clone(),
        }
    }
}

impl From<ListedInstance> for DaemonInstance {
    fn from(listed: ListedInstance) -> Self {
        let listing = DaemonInstanceListing::from(&listed);
        let runtime_state = listed
            .state
            .filter(|_| listing.run_state == ContainerRunState::Running);
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
            listing,
            memory_in_use: runtime_state
                .and_then(|runtime_state| runtime_state.memory)
                .map(|memory_state| ByteSize::b(memory_state.usage))
                .filter(|usage| usage.as_u64() > 0),
            global_addresses,
            image_description: listed.config.get("image.description").cloned(),
        }
    }
}

async fn read_daemon_instances(
    socket_path: &Path,
) -> Result<FamilyDetails<DaemonInstance>, EngineReadError> {
    let listing: Result<InstanceListingReply, _> = http_json_client::get_json_over_unix_socket(
        socket_path,
        &format!("{INCUS_API_ROOT}/instances?recursion=2&all-projects=true"),
    )
    .await;
    match listing {
        Ok(listing) => {
            let mut details: Vec<DaemonInstance> = listing
                .metadata
                .into_iter()
                .map(DaemonInstance::from)
                .collect();
            details.sort_by(|first, second| {
                first
                    .listing
                    .instance_name
                    .cmp(&second.listing.instance_name)
            });
            Ok(FamilyDetails { details })
        }
        Err(fetch_failure) => Err(classified_engine_failure(socket_path, fetch_failure).await),
    }
}

impl SnapshotSource for FamilyDetails<DaemonInstance> {
    type Error = EngineReadError;
    type StreamInputs = InstanceDaemon;

    const NAME: &'static str = "Incus instances";

    fn delivery_stream(
        instance_daemon: InstanceDaemon,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let watched_daemon = instance_daemon;
        path_present(instance_daemon.socket_path()).while_available(
            move || {
                let watched_daemon = watched_daemon;
                PeriodicPolling::every(INSTANCE_POLL_PERIOD)
                    .poll_deliveries(move || async move {
                        read_daemon_instances(&watched_daemon.socket_path()).await
                    })
                    .map(unavailable_when_socket_missing)
            },
            || SnapshotDelivery::SourceUnavailable,
        )
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;
    use serde_json::json;

    use super::{DaemonInstance, InstanceKind, InstanceListingReply};
    use crate::container_run_state::ContainerRunState;

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
        assert_eq!(instances[1].listing.kind, InstanceKind::VirtualMachine);
        assert_eq!(instances[1].listing.run_state, ContainerRunState::Stopped);
        assert_eq!(instances[1].memory_in_use, None);
    }
}
