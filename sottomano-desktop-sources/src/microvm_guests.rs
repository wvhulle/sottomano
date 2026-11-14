pub mod hypervisor_command_lines;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use bytesize::ByteSize;
use futures::Stream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{ChangeWatchFailure, PeriodicPolling},
    microvm_guests::hypervisor_command_lines::{
        HypervisorLaunch, MicrovmHypervisor, launched_hypervisor, parsed_launch,
        with_firecracker_config,
    },
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses,
        procfs_process_scan::scan_processes,
        procfs_process_tree::ProcessTable,
        sysfs_attribute_reads::Error,
        sysfs_cgroup_memory::{
            cgroup_directory_of_process, read_cgroup_memory, unit_cgroup_enclosing,
        },
    },
    systemd_units_journal::template_instance_units::listed_template_instances,
};
const MACHINE_POLL_PERIOD: Duration = Duration::from_secs(5);
const PRESENCE_POLL_PERIOD: Duration = Duration::from_secs(30);
const MICROVM_UNIT_PREFIX: &str = "microvm@";
const MICROVM_UNIT_TEMPLATES: [&str; 1] = ["microvm"];
const SERVICE_UNIT_SUFFIX: &str = ".service";
const PROC_ROOT: &str = "/proc";
const COMM_FILE_NAME: &str = "comm";
const CLOUD_HYPERVISOR_COMM_PREFIX: &str = "cloud-hypervis";
const HYPERVISOR_COMM_NAMES: [&str; 4] = ["firecracker", "crosvm", "lkvm", "kvmtool"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicrovmProbe {
    pub running_guest_processes: usize,
}

impl SnapshotSource for MicrovmProbe {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "MicroVM presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(PRESENCE_POLL_PERIOD).blocking_poll_deliveries(|| {
            Ok(MicrovmProbe {
                running_guest_processes: counted_hypervisor_processes(Path::new(PROC_ROOT)),
            })
        })
    }
}

fn hypervisor_comm_name_matches(comm_name: &str) -> bool {
    let comm_name = comm_name.trim();
    let unwrapped_name = comm_name.strip_prefix('.').unwrap_or(comm_name);
    HYPERVISOR_COMM_NAMES.contains(&unwrapped_name)
        || unwrapped_name.starts_with(CLOUD_HYPERVISOR_COMM_PREFIX)
}

fn counted_hypervisor_processes(proc_root: &Path) -> usize {
    fs::read_dir(proc_root)
        .inspect_err(|listing_failure| {
            tracing::warn!(
                "microvm guests: cannot list {}: {listing_failure}",
                proc_root.display()
            );
        })
        .into_iter()
        .flatten()
        .filter_map(|process_entry| {
            process_entry
                .inspect_err(|entry_failure| {
                    tracing::debug!(
                        "microvm guests: skipping unreadable entry of {}: {entry_failure}",
                        proc_root.display()
                    );
                })
                .ok()
        })
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|process_id| process_id.bytes().all(|digit| digit.is_ascii_digit()))
        })
        .filter_map(|entry| {
            let comm_path = entry.path().join(COMM_FILE_NAME);
            fs::read_to_string(&comm_path)
                .inspect_err(|read_failure| {
                    tracing::debug!(
                        "microvm guests: cannot read {}: {read_failure}",
                        comm_path.display()
                    );
                })
                .ok()
        })
        .filter(|comm_name| hypervisor_comm_name_matches(comm_name))
        .count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrovmGuestListing {
    pub pid: i32,
    pub machine_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrovmGuestListings {
    pub listings: Vec<MicrovmGuestListing>,
}

impl SnapshotSource for MicrovmGuestListings {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "MicroVM machine names";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(MACHINE_POLL_PERIOD).blocking_poll_deliveries(listed_machines)
    }
}

fn listed_machines() -> Result<MicrovmGuestListings, Error> {
    let process_scan =
        scan_processes(|scanned| launched_hypervisor(&scanned.command_line).is_some())
            .ok_or_else(|| Error::Unreadable(PROC_ROOT.to_owned()))?;
    let mut listings: Vec<MicrovmGuestListing> = process_scan
        .matched_processes
        .iter()
        .filter_map(|scanned| {
            let hypervisor = launched_hypervisor(&scanned.command_line)?;
            Some(MicrovmGuestListing {
                pid: scanned.pid,
                machine_name: parsed_launch(hypervisor, &scanned.command_line)
                    .named_machine
                    .unwrap_or_else(|| format!("{} {}", hypervisor.product_name(), scanned.pid)),
            })
        })
        .collect();
    listings.sort_by(|first, second| first.machine_name.cmp(&second.machine_name));
    Ok(MicrovmGuestListings { listings })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrovmGuest {
    pub pid: i32,
    pub machine_name: String,
    pub hypervisor: MicrovmHypervisor,
    pub unit_name: Option<String>,
    pub launch: HypervisorLaunch,
    pub started_at: SystemTime,
    pub memory_in_use: ByteSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrovmGuests {
    pub machines: Vec<MicrovmGuest>,
}

impl SnapshotSource for MicrovmGuests {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "MicroVMs";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(MACHINE_POLL_PERIOD).blocking_poll_deliveries(scanned_machines)
    }
}

fn enclosing_unit_name(pid: i32) -> Option<String> {
    let process_cgroup = cgroup_directory_of_process(pid.unsigned_abs())?;
    let unit_cgroup = unit_cgroup_enclosing(&process_cgroup)?;
    let unit_directory_name = unit_cgroup.file_name()?;
    Some(
        unit_directory_name
            .to_str()
            .expect("systemd escapes unit names to ascii")
            .to_owned(),
    )
}

fn microvm_instance_name(unit_name: &str) -> Option<&str> {
    unit_name
        .strip_prefix(MICROVM_UNIT_PREFIX)?
        .strip_suffix(SERVICE_UNIT_SUFFIX)
}

struct HypervisorProcess {
    pid: i32,
    hypervisor: MicrovmHypervisor,
    launch: HypervisorLaunch,
    started_at: SystemTime,
}

fn shown_machine(
    hypervisor_process: HypervisorProcess,
    process_table: &ProcessTable,
) -> MicrovmGuest {
    let unit_name = enclosing_unit_name(hypervisor_process.pid);
    let microvm_name = unit_name.as_deref().and_then(microvm_instance_name);
    let memory_in_use = unit_name
        .as_deref()
        .filter(|_| microvm_name.is_some())
        .and_then(|_| cgroup_directory_of_process(hypervisor_process.pid.unsigned_abs()))
        .and_then(|process_cgroup| {
            read_cgroup_memory(unit_cgroup_enclosing(&process_cgroup)?)
                .map(|reading| reading.current)
        })
        .unwrap_or_else(|| {
            ByteSize::b(
                process_table
                    .tree_totals(hypervisor_process.pid)
                    .resident_bytes,
            )
        });
    let machine_name = microvm_name
        .map(str::to_owned)
        .or_else(|| hypervisor_process.launch.named_machine.clone())
        .unwrap_or_else(|| {
            format!(
                "{} {}",
                hypervisor_process.hypervisor.product_name(),
                hypervisor_process.pid
            )
        });
    MicrovmGuest {
        pid: hypervisor_process.pid,
        machine_name,
        hypervisor: hypervisor_process.hypervisor,
        unit_name,
        launch: hypervisor_process.launch,
        started_at: hypervisor_process.started_at,
        memory_in_use,
    }
}

fn read_launch(hypervisor: MicrovmHypervisor, command_line: &[String]) -> HypervisorLaunch {
    let launch = parsed_launch(hypervisor, command_line);
    match launch.config_file.clone() {
        Some(config_file) if hypervisor == MicrovmHypervisor::Firecracker => {
            match fs::read_to_string(&config_file) {
                Ok(config_text) => with_firecracker_config(launch, &config_text),
                Err(_) => launch,
            }
        }
        Some(_) | None => launch,
    }
}

fn scanned_machines() -> Result<MicrovmGuests, Error> {
    let process_scan =
        scan_processes(|scanned| launched_hypervisor(&scanned.command_line).is_some())
            .ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let mut machines: Vec<MicrovmGuest> = process_scan
        .matched_processes
        .iter()
        .filter_map(|scanned| {
            let hypervisor = launched_hypervisor(&scanned.command_line)?;
            Some(HypervisorProcess {
                pid: scanned.pid,
                hypervisor,
                launch: read_launch(hypervisor, &scanned.command_line),
                started_at: scanned.started_at,
            })
        })
        .map(|hypervisor_process| shown_machine(hypervisor_process, &process_scan.process_table))
        .collect();
    machines.sort_by(|first, second| first.machine_name.cmp(&second.machine_name));
    Ok(MicrovmGuests { machines })
}

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrovmUnit {
    pub instance_name: String,
    pub unit_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MicrovmUnits {
    pub units: Vec<MicrovmUnit>,
}

impl SnapshotSource for MicrovmUnits {
    type Error = ChangeWatchFailure;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "MicroVM units";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system_bus = buses.system;
        PeriodicPolling::every(MACHINE_POLL_PERIOD).poll_deliveries(move || {
            let system_bus = system_bus.clone();
            async move {
                let units = listed_template_instances(&system_bus, &MICROVM_UNIT_TEMPLATES)
                    .await?
                    .into_iter()
                    .map(|template_unit| MicrovmUnit {
                        instance_name: template_unit.instance_name,
                        unit_name: template_unit.unit_name,
                    })
                    .collect();
                Ok(MicrovmUnits { units })
            }
        })
    }
}

#[must_use]
pub fn api_socket_of(machine: &MicrovmGuest) -> Option<&PathBuf> {
    machine
        .launch
        .api_socket
        .as_ref()
        .filter(|_| machine.hypervisor.has_power_button_api())
}

#[cfg(test)]
mod tests {
    use super::{hypervisor_comm_name_matches, microvm_instance_name};

    #[test]
    fn a_microvm_unit_names_its_machine() {
        assert_eq!(
            microvm_instance_name("microvm@ci-builder.service"),
            Some("ci-builder")
        );
        assert_eq!(microvm_instance_name("user@1000.service"), None);
    }

    #[test]
    fn hypervisor_comms_are_matched_by_name() {
        assert!(hypervisor_comm_name_matches("firecracker\n"));
        assert!(hypervisor_comm_name_matches("cloud-hypervis"));
        assert!(hypervisor_comm_name_matches(".cloud-hypervis"));
        assert!(!hypervisor_comm_name_matches("qemu-system-x86_64"));
        assert!(!hypervisor_comm_name_matches("systemd\n"));
    }
}
