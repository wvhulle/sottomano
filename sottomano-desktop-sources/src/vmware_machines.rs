use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use bytesize::ByteSize;
use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    container_family_snapshots::{FamilyDetails, FamilyListings},
    delivery_cadence_drivers::PeriodicPolling,
    guest_machine_sizing::{GuestCoreCount, GuestMemorySize},
    system_protocol_clients::{
        procfs_process_scan::{program_stem, readable_text, scan_processes},
        sysfs_attribute_reads::FileReadError,
    },
};

const MACHINE_POLL_PERIOD: Duration = Duration::from_secs(5);
const PROBE_POLL_PERIOD: Duration = Duration::from_secs(30);
const VMX_PROGRAM: &str = "vmware-vmx";
const VMX_EXTENSION: &str = ".vmx";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmwareMachineProbe {
    pub running_count: usize,
}

impl SnapshotSource for VmwareMachineProbe {
    type Error = FileReadError;
    type StreamInputs = ();

    const NAME: &'static str = "VMware presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(PROBE_POLL_PERIOD).blocking_poll_deliveries(probed_machines)
    }
}

fn probed_machines() -> Result<VmwareMachineProbe, FileReadError> {
    let process_scan =
        scan_processes(|scanned| program_stem(&scanned.command_line) == Some(VMX_PROGRAM))
            .ok_or_else(|| FileReadError::Unreadable("/proc".to_owned()))?;
    Ok(VmwareMachineProbe {
        running_count: process_scan.matched_processes.len(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmwareMachineListing {
    pub pid: i32,
    pub machine_name: String,
    pub configuration_file: PathBuf,
}

impl SnapshotSource for FamilyListings<VmwareMachineListing> {
    type Error = FileReadError;
    type StreamInputs = ();

    const NAME: &'static str = "VMware machine names";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(MACHINE_POLL_PERIOD).blocking_poll_deliveries(named_machines)
    }
}

fn named_machines() -> Result<FamilyListings<VmwareMachineListing>, FileReadError> {
    let process_scan =
        scan_processes(|scanned| program_stem(&scanned.command_line) == Some(VMX_PROGRAM))
            .ok_or_else(|| FileReadError::Unreadable("/proc".to_owned()))?;
    let mut listings: Vec<VmwareMachineListing> = process_scan
        .matched_processes
        .iter()
        .filter_map(|scanned| {
            let configuration_file = vmx_configuration_file(&scanned.command_line)?;
            Some(VmwareMachineListing {
                pid: scanned.pid,
                machine_name: file_stem_text(&configuration_file),
                configuration_file,
            })
        })
        .collect();
    listings.sort_by(|first, second| first.machine_name.cmp(&second.machine_name));
    Ok(FamilyListings { listings })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmwareGuestOs {
    Declared(String),
    Undeclared,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmwareMachine {
    pub listing: VmwareMachineListing,
    pub guest_os: VmwareGuestOs,
    pub guest_memory: GuestMemorySize,
    pub guest_cores: GuestCoreCount,
    pub started_at: SystemTime,
    pub memory_in_use: ByteSize,
}

impl SnapshotSource for FamilyDetails<VmwareMachine> {
    type Error = FileReadError;
    type StreamInputs = ();

    const NAME: &'static str = "VMware machines";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(MACHINE_POLL_PERIOD).blocking_poll_deliveries(scanned_machines)
    }
}

fn parsed_vmx_entries(vmx_text: &str) -> HashMap<String, String> {
    vmx_text
        .lines()
        .filter_map(|vmx_line| vmx_line.split_once('='))
        .map(|(vmx_key, vmx_value)| {
            (
                vmx_key.trim().to_lowercase(),
                vmx_value.trim().trim_matches('"').to_owned(),
            )
        })
        .collect()
}

fn vmx_configuration_file(command_line: &[String]) -> Option<PathBuf> {
    command_line
        .iter()
        .rev()
        .find(|argument| argument.ends_with(VMX_EXTENSION))
        .map(PathBuf::from)
}

fn file_stem_text(configuration_file: &Path) -> String {
    configuration_file
        .file_stem()
        .map_or_else(String::new, |file_stem| file_stem.display().to_string())
}

fn scanned_machines() -> Result<FamilyDetails<VmwareMachine>, FileReadError> {
    let process_scan =
        scan_processes(|scanned| program_stem(&scanned.command_line) == Some(VMX_PROGRAM))
            .ok_or_else(|| FileReadError::Unreadable("/proc".to_owned()))?;
    let mut details: Vec<VmwareMachine> = process_scan
        .matched_processes
        .iter()
        .filter_map(|scanned| {
            let configuration_file = vmx_configuration_file(&scanned.command_line)?;
            let vmx_entries = readable_text(&configuration_file)
                .map(|vmx_text| parsed_vmx_entries(&vmx_text))
                .unwrap_or_default();
            Some(VmwareMachine {
                listing: VmwareMachineListing {
                    pid: scanned.pid,
                    machine_name: vmx_entries
                        .get("displayname")
                        .cloned()
                        .unwrap_or_else(|| file_stem_text(&configuration_file)),
                    configuration_file: configuration_file.clone(),
                },
                guest_os: vmx_entries
                    .get("guestos")
                    .cloned()
                    .map_or(VmwareGuestOs::Undeclared, VmwareGuestOs::Declared),
                guest_memory: vmx_entries
                    .get("memsize")
                    .and_then(|memory_mebibytes| {
                        memory_mebibytes
                            .parse()
                            .inspect_err(|parse_failure| {
                                tracing::warn!(
                                    "vmware: memsize {memory_mebibytes} in {} is malformed: \
                                     {parse_failure}",
                                    configuration_file.display()
                                );
                            })
                            .ok()
                    })
                    .map_or(GuestMemorySize::HypervisorDefault, |memory_mebibytes| {
                        GuestMemorySize::Configured(ByteSize::mib(memory_mebibytes))
                    }),
                guest_cores: vmx_entries
                    .get("numvcpus")
                    .and_then(|cpu_count| {
                        cpu_count
                            .parse()
                            .inspect_err(|parse_failure| {
                                tracing::warn!(
                                    "vmware: numvcpus {cpu_count} in {} is malformed: \
                                     {parse_failure}",
                                    configuration_file.display()
                                );
                            })
                            .ok()
                    })
                    .map_or(
                        GuestCoreCount::HypervisorDefault,
                        GuestCoreCount::Configured,
                    ),
                started_at: scanned.started_at,
                memory_in_use: ByteSize::b(
                    process_scan
                        .process_table
                        .tree_totals(scanned.pid)
                        .resident_bytes,
                ),
            })
        })
        .collect();
    details.sort_by(|first, second| first.listing.machine_name.cmp(&second.listing.machine_name));
    Ok(FamilyDetails { details })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{parsed_vmx_entries, vmx_configuration_file};

    #[test]
    fn a_vmx_file_names_its_machine_size_and_guest() {
        let vmx_entries = parsed_vmx_entries(
            ".encoding = \"UTF-8\"\ndisplayName = \"Ubuntu 26.04\"\nmemsize = \"4096\"\nnumvcpus \
             = \"2\"\nguestOS = \"ubuntu-64\"\n",
        );
        assert_eq!(vmx_entries["displayname"], "Ubuntu 26.04");
        assert_eq!(vmx_entries["memsize"], "4096");
        assert_eq!(vmx_entries["guestos"], "ubuntu-64");
    }

    #[test]
    fn the_configuration_file_is_the_last_vmx_argument() {
        let command_line: Vec<String> = [
            "/usr/lib/vmware/bin/vmware-vmx",
            "-s",
            "vmx.stdio.keep=TRUE",
            "-#",
            "product=1;name=VMware Workstation",
            "-@",
            "duplex=3;msgs=ui",
            "/home/user/vmware/Ubuntu/Ubuntu.vmx",
        ]
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect();
        assert_eq!(
            vmx_configuration_file(&command_line),
            Some(PathBuf::from("/home/user/vmware/Ubuntu/Ubuntu.vmx"))
        );
    }
}
