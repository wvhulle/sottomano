use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use bytesize::ByteSize;
use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    container_family_snapshots::{FamilyDetails, FamilyListings},
    delivery_cadence_drivers::PeriodicPolling,
    qemu_virtual_machines::procfs_qemu_processes::account_names_by_uid,
    system_protocol_clients::{
        procfs_process_scan::{ScannedProcess, scan_processes},
        sysfs_attribute_reads::FileReadError,
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
    type Error = FileReadError;
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
}

impl SnapshotSource for FamilyListings<ApptainerContainerListing> {
    type Error = FileReadError;
    type StreamInputs = ();

    const NAME: &'static str = "Apptainer container names";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(CONTAINER_POLL_PERIOD)
            .blocking_poll_deliveries(named_runtime_parents)
    }
}

fn named_runtime_parents() -> Result<FamilyListings<ApptainerContainerListing>, FileReadError> {
    let process_scan = scan_processes(is_runtime_parent)
        .ok_or_else(|| FileReadError::Unreadable("/proc".to_owned()))?;
    let listings = process_scan
        .matched_processes
        .iter()
        .map(|runtime_parent| {
            let container_process = process_scan
                .all_processes
                .iter()
                .find(|scanned| scanned.parent_pid == runtime_parent.pid);
            let image_file = container_process
                .map_or(ApptainerImage::NotLoopMounted, |container| {
                    loop_mounted_image(container.pid)
                });
            ApptainerContainerListing {
                runtime_pid: runtime_parent.pid,
                container_name: named_after_image(runtime_parent.pid, &image_file),
            }
        })
        .collect();
    Ok(FamilyListings { listings })
}

fn named_after_image(runtime_pid: i32, image_file: &ApptainerImage) -> String {
    match image_file {
        ApptainerImage::LoopMounted(image_path) => image_path.file_stem().map_or_else(
            || format!("container {runtime_pid}"),
            |image_stem| image_stem.display().to_string(),
        ),
        ApptainerImage::NotLoopMounted => format!("container {runtime_pid}"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApptainerImage {
    LoopMounted(PathBuf),
    NotLoopMounted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainedCommand {
    Running(String),
    Unobserved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerOwner {
    Account(String),
    UnnamedUid(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApptainerContainer {
    pub listing: ApptainerContainerListing,
    pub runtime_title: String,
    pub image_file: ApptainerImage,
    pub running_command: ContainedCommand,
    pub owner: ContainerOwner,
    pub started_at: SystemTime,
    pub memory_in_use: ByteSize,
}

impl SnapshotSource for FamilyDetails<ApptainerContainer> {
    type Error = FileReadError;
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

fn loop_mounted_image(container_pid: i32) -> ApptainerImage {
    loop_backing_file(container_pid)
        .map_or(ApptainerImage::NotLoopMounted, ApptainerImage::LoopMounted)
}

fn loop_backing_file(container_pid: i32) -> Option<PathBuf> {
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

fn scanned_containers() -> Result<FamilyDetails<ApptainerContainer>, FileReadError> {
    let process_scan = scan_processes(is_runtime_parent)
        .ok_or_else(|| FileReadError::Unreadable("/proc".to_owned()))?;
    let account_names = if process_scan.matched_processes.is_empty() {
        HashMap::new()
    } else {
        account_names_by_uid()
    };
    let mut details: Vec<ApptainerContainer> = process_scan
        .matched_processes
        .iter()
        .map(|runtime_parent| {
            let container_process = process_scan
                .all_processes
                .iter()
                .find(|scanned| scanned.parent_pid == runtime_parent.pid);
            let image_file = container_process
                .map_or(ApptainerImage::NotLoopMounted, |container| {
                    loop_mounted_image(container.pid)
                });
            ApptainerContainer {
                listing: ApptainerContainerListing {
                    runtime_pid: runtime_parent.pid,
                    container_name: named_after_image(runtime_parent.pid, &image_file),
                },
                runtime_title: runtime_parent
                    .command_line
                    .first()
                    .cloned()
                    .unwrap_or_else(|| runtime_parent.command_name.clone()),
                image_file,
                running_command: container_process
                    .map(|container| container.command_line.join(" "))
                    .filter(|running_command| !running_command.is_empty())
                    .map_or(ContainedCommand::Unobserved, ContainedCommand::Running),
                owner: account_names
                    .get(&runtime_parent.owner_uid)
                    .cloned()
                    .map_or(
                        ContainerOwner::UnnamedUid(runtime_parent.owner_uid),
                        ContainerOwner::Account,
                    ),
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
    details.sort_by(|first, second| {
        first
            .listing
            .container_name
            .cmp(&second.listing.container_name)
    });
    Ok(FamilyDetails { details })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{ApptainerImage, loop_device_backing_files, named_after_image};

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
        assert_eq!(
            named_after_image(
                5120,
                &ApptainerImage::LoopMounted(PathBuf::from("/home/user/images/lolcow_latest.sif"))
            ),
            "lolcow_latest"
        );
        assert_eq!(
            named_after_image(5120, &ApptainerImage::NotLoopMounted),
            "container 5120"
        );
    }
}
