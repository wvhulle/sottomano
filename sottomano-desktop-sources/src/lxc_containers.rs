use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    future::ready,
    path::{Path, PathBuf},
    time::Duration,
};

use bytesize::ByteSize;
use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeTriggerStreams, PeriodicPolling},
    system_protocol_clients::{
        inotify_directory_changes::{WatchTarget, changes},
        sysfs_attribute_reads::Error,
        sysfs_cgroup_memory::{CGROUP_ROOT, read_cgroup_memory},
    },
};

const CONTAINER_POLL_PERIOD: Duration = Duration::from_secs(5);
const DEFAULT_LXC_PATH: &str = "/var/lib/lxc";
const PAYLOAD_CGROUP_PREFIX: &str = "lxc.payload.";
const CONFIG_FILE_NAME: &str = "config";
const ADDRESS_KEY_SUFFIX: &str = ".ipv4.address";
const EXCLUDED_CONTAINERS: [&str; 1] = ["waydroid"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxcContainer {
    pub container_name: String,
    pub running: bool,
    pub memory_in_use: Option<ByteSize>,
    pub configured_addresses: Vec<String>,
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxcProbe {
    pub running_count: usize,
    pub configured_count: usize,
}

impl SnapshotSource for LxcProbe {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "LXC presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries((), |()| {
            ready(Ok(changes(vec![
                WatchTarget::EntriesOf(Path::new(DEFAULT_LXC_PATH).to_path_buf()),
                WatchTarget::EntriesOf(Path::new(CGROUP_ROOT).to_path_buf()),
            ])
            .triggered_rereads(|| {
                ready(Ok(counted_lxc_containers(
                    Path::new(DEFAULT_LXC_PATH),
                    Path::new(CGROUP_ROOT),
                )))
            })))
        })
    }
}

fn counted_lxc_containers(lxc_path: &Path, cgroup_root: &Path) -> LxcProbe {
    LxcProbe {
        running_count: running_payload_cgroups(cgroup_root)
            .keys()
            .filter(|container_name| !EXCLUDED_CONTAINERS.contains(&container_name.as_str()))
            .count(),
        configured_count: configured_container_directories(lxc_path)
            .keys()
            .filter(|container_name| !EXCLUDED_CONTAINERS.contains(&container_name.as_str()))
            .count(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxcContainerListing {
    pub container_name: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxcContainerListings {
    pub listings: Vec<LxcContainerListing>,
}

impl SnapshotSource for LxcContainerListings {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "LXC container names";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(CONTAINER_POLL_PERIOD).blocking_poll_deliveries(|| {
            Ok(named_lxc_containers(
                Path::new(DEFAULT_LXC_PATH),
                Path::new(CGROUP_ROOT),
            ))
        })
    }
}

fn named_lxc_containers(lxc_path: &Path, cgroup_root: &Path) -> LxcContainerListings {
    let running_cgroups = running_payload_cgroups(cgroup_root);
    let config_paths = configured_container_directories(lxc_path);
    let listings = running_cgroups
        .keys()
        .chain(config_paths.keys())
        .filter(|container_name| !EXCLUDED_CONTAINERS.contains(&container_name.as_str()))
        .cloned()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .map(|container_name| {
            let running = running_cgroups.contains_key(&container_name);
            LxcContainerListing {
                container_name,
                running,
            }
        })
        .collect();
    LxcContainerListings { listings }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxcContainers {
    pub containers: Vec<LxcContainer>,
}

impl SnapshotSource for LxcContainers {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "LXC containers";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(CONTAINER_POLL_PERIOD).blocking_poll_deliveries(|| {
            Ok(listed_containers(
                Path::new(DEFAULT_LXC_PATH),
                Path::new(CGROUP_ROOT),
            ))
        })
    }
}

fn configured_addresses_in(config_text: &str) -> Vec<String> {
    config_text
        .lines()
        .filter_map(|config_line| config_line.split_once('='))
        .filter(|(config_key, _)| config_key.trim().ends_with(ADDRESS_KEY_SUFFIX))
        .map(|(_, config_value)| config_value.trim().to_owned())
        .collect()
}

fn running_payload_cgroups(cgroup_root: &Path) -> BTreeMap<String, PathBuf> {
    fs::read_dir(cgroup_root)
        .into_iter()
        .flatten()
        .filter_map(|listed_entry| {
            listed_entry
                .inspect_err(|list_failure| {
                    tracing::debug!(
                        "lxc: listing {} failed: {list_failure}",
                        cgroup_root.display()
                    );
                })
                .ok()
        })
        .filter_map(|entry| {
            let directory_name = entry.file_name().to_string_lossy().into_owned();
            let container_name = directory_name.strip_prefix(PAYLOAD_CGROUP_PREFIX)?;
            Some((container_name.to_owned(), entry.path()))
        })
        .collect()
}

fn configured_container_directories(lxc_path: &Path) -> BTreeMap<String, PathBuf> {
    fs::read_dir(lxc_path)
        .into_iter()
        .flatten()
        .filter_map(|listed_entry| {
            listed_entry
                .inspect_err(|list_failure| {
                    tracing::debug!("lxc: listing {} failed: {list_failure}", lxc_path.display());
                })
                .ok()
        })
        .filter(|entry| entry.path().join(CONFIG_FILE_NAME).is_file())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.path().join(CONFIG_FILE_NAME),
            )
        })
        .collect()
}

fn listed_containers(lxc_path: &Path, cgroup_root: &Path) -> LxcContainers {
    let mut running_cgroups = running_payload_cgroups(cgroup_root);
    let mut config_paths = configured_container_directories(lxc_path);
    let container_names: Vec<String> = running_cgroups
        .keys()
        .chain(config_paths.keys())
        .filter(|container_name| !EXCLUDED_CONTAINERS.contains(&container_name.as_str()))
        .cloned()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();
    let containers = container_names
        .into_iter()
        .map(|container_name| {
            let payload_cgroup = running_cgroups.remove(&container_name);
            let config_path = config_paths.remove(&container_name);
            LxcContainer {
                running: payload_cgroup.is_some(),
                memory_in_use: payload_cgroup
                    .as_deref()
                    .and_then(read_cgroup_memory)
                    .map(|reading| reading.current),
                configured_addresses: config_path
                    .as_deref()
                    .and_then(|config_path| {
                        fs::read_to_string(config_path)
                            .inspect_err(|read_failure| {
                                tracing::debug!(
                                    "lxc: reading {} failed: {read_failure}",
                                    config_path.display()
                                );
                            })
                            .ok()
                    })
                    .map(|config_text| configured_addresses_in(&config_text))
                    .unwrap_or_default(),
                config_path,
                container_name,
            }
        })
        .collect();
    LxcContainers { containers }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use bytesize::ByteSize;

    use super::{configured_addresses_in, listed_containers};

    #[test]
    fn a_static_address_is_read_from_the_container_config() {
        let config_text =
            "lxc.uts.name = media\nlxc.net.0.type = veth\nlxc.net.0.ipv4.address = 10.0.3.15/24\n";
        assert_eq!(configured_addresses_in(config_text), ["10.0.3.15/24"]);
    }

    #[test]
    fn running_and_stopped_containers_merge_and_waydroid_is_left_out() {
        let scratch = env::temp_dir().join(format!("sottomano-lxc-{}", process::id()));
        let lxc_path = scratch.join("lxc");
        let cgroup_root = scratch.join("cgroup");
        for container_name in ["media", "backup"] {
            fs::create_dir_all(lxc_path.join(container_name)).unwrap();
            fs::write(lxc_path.join(container_name).join("config"), "").unwrap();
        }
        for payload in ["lxc.payload.media", "lxc.payload.waydroid"] {
            fs::create_dir_all(cgroup_root.join(payload)).unwrap();
            fs::write(
                cgroup_root.join(payload).join("memory.current"),
                "52428800\n",
            )
            .unwrap();
        }
        let listed = listed_containers(&lxc_path, &cgroup_root);
        fs::remove_dir_all(&scratch).unwrap();
        let summary: Vec<(&str, bool, Option<ByteSize>)> = listed
            .containers
            .iter()
            .map(|container| {
                (
                    container.container_name.as_str(),
                    container.running,
                    container.memory_in_use,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("backup", false, None),
                ("media", true, Some(ByteSize::mib(50)))
            ]
        );
    }
}
