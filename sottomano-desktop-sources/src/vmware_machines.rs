use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use bytesize::ByteSize;
use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::{
        procfs_process_scan::{program_stem, readable_text, scan_processes},
        sysfs_attribute_reads::Error,
    },
};

const MACHINE_POLL_PERIOD: Duration = Duration::from_secs(5);
const PROBE_POLL_PERIOD: Duration = Duration::from_secs(30);
const VMX_PROGRAM: &str = "vmware-vmx";
const VMX_EXTENSION: &str = ".vmx";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmwareMachineProbe {
    pub running_count: usize,
    pub running_pids: Vec<i32>,
}

impl SnapshotSource for VmwareMachineProbe {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "VMware presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(PROBE_POLL_PERIOD).blocking_poll_deliveries(probed_machines)
    }
}

fn probed_machines() -> Result<VmwareMachineProbe, Error> {
    let process_scan =
        scan_processes(|scanned| program_stem(&scanned.command_line) == Some(VMX_PROGRAM))
            .ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let mut running_pids: Vec<i32> = process_scan
        .matched_processes
        .iter()
        .map(|scanned| scanned.pid)
        .collect();
    running_pids.sort_unstable();
    let running_count = running_pids.len();
    Ok(VmwareMachineProbe {
        running_count,
        running_pids,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmwareMachineListing {
    pub pid: i32,
    pub machine_name: String,
    pub configuration_file: PathBuf,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmwareMachineListings {
    pub listings: Vec<VmwareMachineListing>,
}

impl SnapshotSource for VmwareMachineListings {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "VMware machine names";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(MACHINE_POLL_PERIOD).blocking_poll_deliveries(named_machines)
    }
}

fn named_machines() -> Result<VmwareMachineListings, Error> {
    let process_scan =
        scan_processes(|scanned| program_stem(&scanned.command_line) == Some(VMX_PROGRAM))
            .ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let mut listings: Vec<VmwareMachineListing> = process_scan
        .matched_processes
        .iter()
        .filter_map(|scanned| {
            let configuration_file = configuration_file_of(&scanned.command_line)?;
            Some(VmwareMachineListing {
                pid: scanned.pid,
                machine_name: file_stem_text(&configuration_file),
                configuration_file,
                running: true,
            })
        })
        .collect();
    listings.sort_by(|first, second| first.machine_name.cmp(&second.machine_name));
    Ok(VmwareMachineListings { listings })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmwareMachine {
    pub pid: i32,
    pub machine_name: String,
    pub configuration_file: PathBuf,
    pub guest_os: Option<String>,
    pub guest_memory: Option<ByteSize>,
    pub guest_cores: Option<u32>,
    pub started_at: SystemTime,
    pub memory_in_use: ByteSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmwareMachines {
    pub machines: Vec<VmwareMachine>,
}

impl SnapshotSource for VmwareMachines {
    type Error = Error;
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

fn configuration_file_of(command_line: &[String]) -> Option<PathBuf> {
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

fn scanned_machines() -> Result<VmwareMachines, Error> {
    let process_scan =
        scan_processes(|scanned| program_stem(&scanned.command_line) == Some(VMX_PROGRAM))
            .ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let mut machines: Vec<VmwareMachine> = process_scan
        .matched_processes
        .iter()
        .filter_map(|scanned| {
            let configuration_file = configuration_file_of(&scanned.command_line)?;
            let vmx_entries = readable_text(&configuration_file)
                .map(|vmx_text| parsed_vmx_entries(&vmx_text))
                .unwrap_or_default();
            Some(VmwareMachine {
                pid: scanned.pid,
                machine_name: vmx_entries
                    .get("displayname")
                    .cloned()
                    .unwrap_or_else(|| file_stem_text(&configuration_file)),
                guest_os: vmx_entries.get("guestos").cloned(),
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
                    .map(ByteSize::mib),
                guest_cores: vmx_entries.get("numvcpus").and_then(|cpu_count| {
                    cpu_count
                        .parse()
                        .inspect_err(|parse_failure| {
                            tracing::warn!(
                                "vmware: numvcpus {cpu_count} in {} is malformed: {parse_failure}",
                                configuration_file.display()
                            );
                        })
                        .ok()
                }),
                started_at: scanned.started_at,
                memory_in_use: ByteSize::b(
                    process_scan
                        .process_table
                        .tree_totals(scanned.pid)
                        .resident_bytes,
                ),
                configuration_file,
            })
        })
        .collect();
    machines.sort_by(|first, second| first.machine_name.cmp(&second.machine_name));
    Ok(VmwareMachines { machines })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{configuration_file_of, parsed_vmx_entries};

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
            configuration_file_of(&command_line),
            Some(PathBuf::from("/home/user/vmware/Ubuntu/Ubuntu.vmx"))
        );
    }
}
