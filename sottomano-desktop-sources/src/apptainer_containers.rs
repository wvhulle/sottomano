use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use bytesize::ByteSize;
use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    qemu_virtual_machines::procfs_qemu_processes::account_names_by_uid,
    system_protocol_clients::{
        procfs_process_scan::{ScannedProcess, scan_processes},
        sysfs_attribute_reads::Error,
    },
};

const CONTAINER_POLL_PERIOD: Duration = Duration::from_secs(5);
const PROBE_POLL_PERIOD: Duration = Duration::from_secs(30);
const RUNTIME_PARENT_TITLES: [&str; 2] = ["Apptainer runtime parent", "Singularity runtime parent"];
const RUNTIME_PARENT_COMMAND_NAMES: [&str; 2] = ["Apptainer runti", "Singularity run"];
const LOOP_DEVICE_PREFIX: &str = "/dev/loop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApptainerProbe {
    pub running_count: usize,
}

impl SnapshotSource for ApptainerProbe {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Apptainer presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(PROBE_POLL_PERIOD).blocking_poll_deliveries(|| {
            Ok(ApptainerProbe {
                running_count: counted_runtime_parents(),
            })
        })
    }
}

fn counted_runtime_parents() -> usize {
    scan_processes(is_runtime_parent).map_or(0, |process_scan| process_scan.matched_processes.len())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApptainerContainerListing {
    pub runtime_pid: i32,
    pub container_name: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApptainerContainerListings {
    pub listings: Vec<ApptainerContainerListing>,
}

impl SnapshotSource for ApptainerContainerListings {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Apptainer container names";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(CONTAINER_POLL_PERIOD)
            .blocking_poll_deliveries(named_runtime_parents)
    }
}

fn named_runtime_parents() -> Result<ApptainerContainerListings, Error> {
    let process_scan =
        scan_processes(is_runtime_parent).ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let listings = process_scan
        .matched_processes
        .iter()
        .map(|runtime_parent| {
            let container_process = process_scan
                .all_processes
                .iter()
                .find(|scanned| scanned.parent_pid == runtime_parent.pid);
            let image_file = container_process.and_then(|container| image_file_of(container.pid));
            let container_name = image_file.as_deref().and_then(Path::file_stem).map_or_else(
                || format!("container {}", runtime_parent.pid),
                |image_stem| image_stem.to_string_lossy().into_owned(),
            );
            ApptainerContainerListing {
                runtime_pid: runtime_parent.pid,
                container_name,
                running: true,
            }
        })
        .collect();
    Ok(ApptainerContainerListings { listings })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApptainerContainer {
    pub runtime_pid: i32,
    pub runtime_title: String,
    pub image_file: Option<PathBuf>,
    pub running_command: Option<String>,
    pub owner_uid: u32,
    pub owner_name: Option<String>,
    pub started_at: SystemTime,
    pub memory_in_use: ByteSize,
}

impl ApptainerContainer {
    #[must_use]
    pub fn shown_name(&self) -> String {
        self.image_file
            .as_deref()
            .and_then(Path::file_stem)
            .map_or_else(
                || format!("container {}", self.runtime_pid),
                |image_stem| image_stem.to_string_lossy().into_owned(),
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApptainerContainers {
    pub containers: Vec<ApptainerContainer>,
}

impl SnapshotSource for ApptainerContainers {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Apptainer containers";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(CONTAINER_POLL_PERIOD).blocking_poll_deliveries(scanned_containers)
    }
}

fn is_runtime_parent(scanned: &ScannedProcess) -> bool {
    let title_matches = scanned.command_line.first().is_some_and(|first_argument| {
        RUNTIME_PARENT_TITLES
            .iter()
            .any(|runtime_title| first_argument.starts_with(runtime_title))
    });
    title_matches
        || RUNTIME_PARENT_COMMAND_NAMES
            .iter()
            .any(|command_name| scanned.command_name.starts_with(command_name))
}

fn loop_device_backing_files(mountinfo_text: &str) -> impl Iterator<Item = &str> {
    mountinfo_text.lines().filter_map(|mount_line| {
        let (_, after_separator) = mount_line.split_once(" - ")?;
        let mount_source = after_separator.split_whitespace().nth(1)?;
        mount_source.strip_prefix(LOOP_DEVICE_PREFIX)
    })
}

fn image_file_of(container_pid: i32) -> Option<PathBuf> {
    let mountinfo_text = fs::read_to_string(format!("/proc/{container_pid}/mountinfo"))
        .inspect_err(|read_failure| {
            tracing::debug!(
                "apptainer: reading mounts of pid {container_pid} failed: {read_failure}"
            );
        })
        .ok()?;
    loop_device_backing_files(&mountinfo_text).find_map(|loop_number| {
        fs::read_to_string(format!("/sys/block/loop{loop_number}/loop/backing_file"))
            .inspect_err(|read_failure| {
                tracing::debug!(
                    "apptainer: backing file of loop{loop_number} unreadable: {read_failure}"
                );
            })
            .ok()
            .map(|backing_file| PathBuf::from(backing_file.trim()))
    })
}

fn scanned_containers() -> Result<ApptainerContainers, Error> {
    let process_scan =
        scan_processes(is_runtime_parent).ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let account_names = if process_scan.matched_processes.is_empty() {
        HashMap::new()
    } else {
        account_names_by_uid()
    };
    let mut containers: Vec<ApptainerContainer> = process_scan
        .matched_processes
        .iter()
        .map(|runtime_parent| {
            let container_process = process_scan
                .all_processes
                .iter()
                .find(|scanned| scanned.parent_pid == runtime_parent.pid);
            ApptainerContainer {
                runtime_pid: runtime_parent.pid,
                runtime_title: runtime_parent
                    .command_line
                    .first()
                    .cloned()
                    .unwrap_or_else(|| runtime_parent.command_name.clone()),
                image_file: container_process.and_then(|container| image_file_of(container.pid)),
                running_command: container_process
                    .map(|container| container.command_line.join(" "))
                    .filter(|running_command| !running_command.is_empty()),
                owner_uid: runtime_parent.owner_uid,
                owner_name: account_names.get(&runtime_parent.owner_uid).cloned(),
                started_at: runtime_parent.started_at,
                memory_in_use: ByteSize::b(
                    process_scan
                        .process_table
                        .tree_totals(runtime_parent.pid)
                        .resident_bytes,
                ),
            }
        })
        .collect();
    containers.sort_by_key(ApptainerContainer::shown_name);
    Ok(ApptainerContainers { containers })
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::SystemTime};

    use bytesize::ByteSize;

    use super::{ApptainerContainer, loop_device_backing_files};

    #[test]
    fn the_root_image_is_found_through_its_loop_device() {
        let mountinfo_text = "\
            901 900 7:3 / / ro,nodev,relatime - squashfs /dev/loop3 ro\n902 901 0:5 / /dev \
                              rw,nosuid - devtmpfs devtmpfs rw\n";
        assert_eq!(
            loop_device_backing_files(mountinfo_text).collect::<Vec<_>>(),
            ["3"]
        );
    }

    #[test]
    fn a_container_is_named_after_its_image() {
        let container = ApptainerContainer {
            runtime_pid: 5120,
            runtime_title: "Apptainer runtime parent".to_owned(),
            image_file: Some(PathBuf::from("/home/user/images/lolcow_latest.sif")),
            running_command: Some("/bin/bash".to_owned()),
            owner_uid: 1000,
            owner_name: Some("user".to_owned()),
            started_at: SystemTime::UNIX_EPOCH,
            memory_in_use: ByteSize::mib(12),
        };
        assert_eq!(container.shown_name(), "lolcow_latest");
    }
}
