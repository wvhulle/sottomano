use std::{
    collections::HashMap,
    env,
    fs::{self, DirEntry},
    future::ready,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{Stream, StreamExt};
use serde::Deserialize;

use crate::{
    SnapshotDelivery, SnapshotSource,
    container_family_snapshots::{FamilyDetails, FamilyListings},
    container_runtime_facts::{ContainerMemoryUse, ContainerStartTime},
    delivery_cadence_drivers::{
        ChangeTriggerStreams, ChangeWatchFailure, PeriodicPolling, SnapshotDeliveryStreams,
    },
    system_protocol_clients::{
        inotify_directory_changes::{WatchTarget, changes},
        sysfs_attribute_reads::FileReadError,
        sysfs_cgroup_memory::unit_memory_of_process,
    },
};

const SANDBOX_POLL_PERIOD: Duration = Duration::from_secs(5);
const RUNTIME_DIRECTORY_VARIABLE: &str = "XDG_RUNTIME_DIR";
const INSTANCES_DIRECTORY: &str = ".flatpak";
const INFO_FILE_NAME: &str = "info";
const PID_FILE_NAME: &str = "pid";
const BWRAP_INFO_FILE_NAME: &str = "bwrapinfo.json";
const PROC_ROOT: &str = "/proc";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatpakProbe {
    pub running_count: usize,
}

impl SnapshotSource for FlatpakProbe {
    type Error = FileReadError;
    type StreamInputs = ();

    const NAME: &'static str = "Flatpak instance presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        flatpak_instance_changes()
            .triggered_rereads(|| ready(counted_running_instances()))
            .map(SnapshotDelivery::live_push)
            .repeats_as_unchanged()
    }
}

fn flatpak_instance_changes() -> impl Stream<Item = Result<(), ChangeWatchFailure>> + Send {
    changes(
        flatpak_instances_directory()
            .map(WatchTarget::EntriesOf)
            .into_iter()
            .collect(),
    )
}

fn flatpak_instances_directory() -> Option<PathBuf> {
    env::var_os(RUNTIME_DIRECTORY_VARIABLE)
        .map(|runtime_directory| PathBuf::from(runtime_directory).join(INSTANCES_DIRECTORY))
}

fn counted_running_instances() -> Result<FlatpakProbe, FileReadError> {
    let instances_directory = flatpak_instances_directory()
        .ok_or_else(|| FileReadError::Unreadable(RUNTIME_DIRECTORY_VARIABLE.to_owned()))?;
    Ok(FlatpakProbe {
        running_count: running_sandboxes_in(&instances_directory, Path::new(PROC_ROOT))
            .details
            .len(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FlatpakSandboxListing {
    pub application_id: String,
    pub instance_id: String,
}

impl SnapshotSource for FamilyListings<FlatpakSandboxListing> {
    type Error = FileReadError;
    type StreamInputs = ();

    const NAME: &'static str = "Flatpak app ids";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(SANDBOX_POLL_PERIOD).blocking_poll_deliveries(|| {
            let instances_directory = flatpak_instances_directory()
                .ok_or_else(|| FileReadError::Unreadable(RUNTIME_DIRECTORY_VARIABLE.to_owned()))?;
            Ok(listed_sandboxes_in(
                &instances_directory,
                Path::new(PROC_ROOT),
            ))
        })
    }
}

fn listed_sandboxes_in(
    instances_directory: &Path,
    proc_root: &Path,
) -> FamilyListings<FlatpakSandboxListing> {
    let mut listings: Vec<FlatpakSandboxListing> = fs::read_dir(instances_directory)
        .into_iter()
        .flatten()
        .filter_map(|listed_entry| {
            listed_entry
                .inspect_err(|list_failure| {
                    tracing::debug!(
                        "flatpak sandboxes: listing {} failed: {list_failure}",
                        instances_directory.display()
                    );
                })
                .ok()
        })
        .filter_map(|entry| {
            let info_text = instance_info_text(&entry.path())?;
            let instance_info = parsed_instance_info(&info_text)?;
            let sandbox_pid = sandbox_pid_in(&entry.path())?;
            let process_directory = proc_root.join(sandbox_pid.to_string());
            let instance_id = instance_directory_name(&entry)?;
            process_directory.exists().then_some(FlatpakSandboxListing {
                instance_id,
                application_id: instance_info.application_id,
            })
        })
        .collect();
    listings.sort();
    FamilyListings { listings }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatpakSandbox {
    pub listing: FlatpakSandboxListing,
    pub runtime_reference: FlatpakRuntime,
    pub branch: FlatpakBranch,
    pub sandbox_pid: u32,
    pub started_at: ContainerStartTime,
    pub memory_in_use: ContainerMemoryUse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatpakRuntime {
    Reference(String),
    Unstated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlatpakBranch {
    Named(String),
    Unstated,
}

impl SnapshotSource for FamilyDetails<FlatpakSandbox> {
    type Error = FileReadError;
    type StreamInputs = ();

    const NAME: &'static str = "Flatpak sandboxes";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(SANDBOX_POLL_PERIOD).blocking_poll_deliveries(|| {
            let instances_directory = flatpak_instances_directory()
                .ok_or_else(|| FileReadError::Unreadable(RUNTIME_DIRECTORY_VARIABLE.to_owned()))?;
            Ok(running_sandboxes_in(
                &instances_directory,
                Path::new(PROC_ROOT),
            ))
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct InstanceInfo {
    application_id: String,
    runtime_reference: FlatpakRuntime,
    branch: FlatpakBranch,
}

fn parsed_instance_info(info_text: &str) -> Option<InstanceInfo> {
    let mut current_section = "";
    let mut info_values: HashMap<(&str, &str), &str> = HashMap::new();
    for info_line in info_text.lines().map(str::trim) {
        if let Some(section_name) = info_line
            .strip_prefix('[')
            .and_then(|bracketed| bracketed.strip_suffix(']'))
        {
            current_section = section_name;
            continue;
        }
        let Some((info_key, info_value)) = info_line.split_once('=') else {
            continue;
        };
        info_values.insert((current_section, info_key.trim()), info_value.trim());
    }
    let info_value = |section_name: &str, info_key: &str| {
        info_values
            .get(&(section_name, info_key))
            .map(|info_value| (*info_value).to_owned())
    };
    Some(InstanceInfo {
        application_id: info_value("Application", "name")?,
        runtime_reference: info_value("Application", "runtime")
            .map_or(FlatpakRuntime::Unstated, FlatpakRuntime::Reference),
        branch: info_value("Instance", "branch")
            .map_or(FlatpakBranch::Unstated, FlatpakBranch::Named),
    })
}

#[derive(Deserialize)]
struct BubblewrapInfo {
    #[serde(rename = "child-pid")]
    child_pid: u32,
}

fn instance_info_text(instance_directory: &Path) -> Option<String> {
    let info_path = instance_directory.join(INFO_FILE_NAME);
    fs::read_to_string(&info_path)
        .inspect_err(|read_failure| {
            tracing::debug!(
                "flatpak sandboxes: reading {} failed: {read_failure}",
                info_path.display()
            );
        })
        .ok()
}

fn sandbox_pid_in(instance_directory: &Path) -> Option<u32> {
    let pid_path = instance_directory.join(PID_FILE_NAME);
    let bwrap_info_path = instance_directory.join(BWRAP_INFO_FILE_NAME);
    let recorded_pid = match fs::read_to_string(&pid_path) {
        Ok(pid_text) => pid_text
            .trim()
            .parse()
            .inspect_err(|parse_failure| {
                tracing::warn!(
                    "flatpak sandboxes: {} holds no pid: {parse_failure}",
                    pid_path.display()
                );
            })
            .ok(),
        Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => None,
        Err(read_failure) => {
            tracing::debug!(
                "flatpak sandboxes: reading {} failed: {read_failure}",
                pid_path.display()
            );
            None
        }
    };
    recorded_pid.or_else(|| {
        let bwrap_info_text = fs::read_to_string(&bwrap_info_path)
            .inspect_err(|read_failure| {
                tracing::debug!(
                    "flatpak sandboxes: reading {} failed: {read_failure}",
                    bwrap_info_path.display()
                );
            })
            .ok()?;
        serde_json::from_str::<BubblewrapInfo>(&bwrap_info_text)
            .inspect_err(|parse_failure| {
                tracing::warn!(
                    "flatpak sandboxes: {} is malformed: {parse_failure}",
                    bwrap_info_path.display()
                );
            })
            .ok()
            .map(|bwrap_info| bwrap_info.child_pid)
    })
}

fn running_sandboxes_in(
    instances_directory: &Path,
    proc_root: &Path,
) -> FamilyDetails<FlatpakSandbox> {
    let mut details: Vec<FlatpakSandbox> = fs::read_dir(instances_directory)
        .into_iter()
        .flatten()
        .filter_map(|listed_entry| {
            listed_entry
                .inspect_err(|list_failure| {
                    tracing::debug!(
                        "flatpak sandboxes: listing {} failed: {list_failure}",
                        instances_directory.display()
                    );
                })
                .ok()
        })
        .filter_map(|entry| {
            let sandbox_directory = entry.path();
            let info_text = instance_info_text(&sandbox_directory)?;
            let instance_info = parsed_instance_info(&info_text)?;
            let sandbox_pid = sandbox_pid_in(&sandbox_directory)?;
            let process_directory = proc_root.join(sandbox_pid.to_string());
            if !process_directory.exists() {
                return None;
            }
            Some(FlatpakSandbox {
                listing: FlatpakSandboxListing {
                    instance_id: instance_directory_name(&entry)?,
                    application_id: instance_info.application_id,
                },
                runtime_reference: instance_info.runtime_reference,
                branch: instance_info.branch,
                sandbox_pid,
                started_at: fs::metadata(&sandbox_directory)
                    .and_then(|metadata| metadata.modified())
                    .inspect_err(|stat_failure| {
                        tracing::debug!(
                            "flatpak sandboxes: start time of {} unknown: {stat_failure}",
                            sandbox_directory.display()
                        );
                    })
                    .ok()
                    .into(),
                memory_in_use: unit_memory_of_process(sandbox_pid).into(),
            })
        })
        .collect();
    details.sort_by(|first, second| first.listing.cmp(&second.listing));
    FamilyDetails { details }
}

fn instance_directory_name(entry: &DirEntry) -> Option<String> {
    entry
        .file_name()
        .into_string()
        .inspect_err(|non_utf8_name| {
            tracing::warn!(
                "flatpak: skipping instance directory with a non-utf-8 name {}",
                non_utf8_name.display()
            );
        })
        .ok()
}

#[cfg(test)]
mod flatpak_sandboxes_tests;
