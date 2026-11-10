use std::{
    env,
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
const VIRTUALBOX_VM_PROGRAMS: [&str; 3] = ["VBoxHeadless", "VirtualBoxVM", "VBoxSDL"];
const DEFAULT_MACHINE_FOLDER: &str = "VirtualBox VMs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualBoxMachineProbe {
    pub running_count: usize,
    pub running_pids: Vec<i32>,
}

impl SnapshotSource for VirtualBoxMachineProbe {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "VirtualBox presence";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(PROBE_POLL_PERIOD).blocking_poll_deliveries(probed_machines)
    }
}

fn probed_machines() -> Result<VirtualBoxMachineProbe, Error> {
    let process_scan = scan_processes(|scanned| {
        program_stem(&scanned.command_line)
            .is_some_and(|program| VIRTUALBOX_VM_PROGRAMS.contains(&program))
    })
    .ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let mut running_pids: Vec<i32> = process_scan
        .matched_processes
        .iter()
        .map(|scanned| scanned.pid)
        .collect();
    running_pids.sort_unstable();
    let running_count = running_pids.len();
    Ok(VirtualBoxMachineProbe {
        running_count,
        running_pids,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualBoxMachineListing {
    pub pid: i32,
    pub machine_name: String,
    pub machine_reference: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualBoxMachineListings {
    pub listings: Vec<VirtualBoxMachineListing>,
}

impl SnapshotSource for VirtualBoxMachineListings {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "VirtualBox machine names";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(MACHINE_POLL_PERIOD).blocking_poll_deliveries(named_machines)
    }
}

fn named_machines() -> Result<VirtualBoxMachineListings, Error> {
    let process_scan = scan_processes(|scanned| {
        program_stem(&scanned.command_line)
            .is_some_and(|program| VIRTUALBOX_VM_PROGRAMS.contains(&program))
    })
    .ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let mut listings: Vec<VirtualBoxMachineListing> = process_scan
        .matched_processes
        .iter()
        .filter_map(|scanned| {
            let machine_reference =
                option_value(&scanned.command_line, &["--startvm", "-startvm", "-s"])?.to_owned();
            let machine_name = option_value(&scanned.command_line, &["--comment", "-comment"])
                .map_or_else(|| machine_reference.clone(), str::to_owned);
            Some(VirtualBoxMachineListing {
                pid: scanned.pid,
                machine_name,
                machine_reference,
                running: true,
            })
        })
        .collect();
    listings.sort_by(|first, second| first.machine_name.cmp(&second.machine_name));
    Ok(VirtualBoxMachineListings { listings })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualBoxFrontend {
    Headless,
    Windowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualBoxMachine {
    pub pid: i32,
    pub machine_name: String,
    pub machine_reference: String,
    pub frontend: VirtualBoxFrontend,
    pub guest_memory: Option<ByteSize>,
    pub guest_cores: Option<u32>,
    pub settings_file: Option<PathBuf>,
    pub started_at: SystemTime,
    pub memory_in_use: ByteSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualBoxMachines {
    pub machines: Vec<VirtualBoxMachine>,
}

impl SnapshotSource for VirtualBoxMachines {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "VirtualBox machines";

    fn delivery_stream(
        (): Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(MACHINE_POLL_PERIOD).blocking_poll_deliveries(scanned_machines)
    }
}

fn option_value<'a>(command_line: &'a [String], option_names: &[&str]) -> Option<&'a str> {
    command_line
        .windows(2)
        .find(|argument_pair| option_names.contains(&argument_pair[0].as_str()))
        .map(|argument_pair| argument_pair[1].as_str())
}

fn quoted_attribute<'a>(
    settings_text: &'a str,
    element_start: &str,
    attribute_name: &str,
) -> Option<&'a str> {
    let element_text = &settings_text[settings_text.find(element_start)?..];
    let element_text = &element_text[..element_text.find('>')?];
    let attribute_marker = format!("{attribute_name}=\"");
    let after_marker =
        &element_text[element_text.find(&attribute_marker)? + attribute_marker.len()..];
    Some(&after_marker[..after_marker.find('"')?])
}

#[derive(Debug, Default, PartialEq, Eq)]
struct MachineSettings {
    guest_memory: Option<ByteSize>,
    guest_cores: Option<u32>,
}

fn parsed_machine_settings(settings_text: &str) -> MachineSettings {
    MachineSettings {
        guest_memory: quoted_attribute(settings_text, "<Memory ", "RAMSize")
            .and_then(|ram_mebibytes| ram_mebibytes.parse().ok())
            .map(ByteSize::mib),
        guest_cores: quoted_attribute(settings_text, "<CPU", "count")
            .and_then(|cpu_count| cpu_count.parse().ok())
            .or_else(|| settings_text.contains("<CPU").then_some(1)),
    }
}

fn default_settings_file(machine_name: &str) -> Option<PathBuf> {
    let home_directory = PathBuf::from(env::var_os("HOME")?);
    Some(
        home_directory
            .join(DEFAULT_MACHINE_FOLDER)
            .join(machine_name)
            .join(format!("{machine_name}.vbox")),
    )
}

fn scanned_machines() -> Result<VirtualBoxMachines, Error> {
    let process_scan = scan_processes(|scanned| {
        program_stem(&scanned.command_line)
            .is_some_and(|program| VIRTUALBOX_VM_PROGRAMS.contains(&program))
    })
    .ok_or_else(|| Error::Unreadable("/proc".to_owned()))?;
    let mut machines: Vec<VirtualBoxMachine> = process_scan
        .matched_processes
        .iter()
        .filter_map(|scanned| {
            let machine_reference =
                option_value(&scanned.command_line, &["--startvm", "-startvm", "-s"])?.to_owned();
            let machine_name = option_value(&scanned.command_line, &["--comment", "-comment"])
                .map_or_else(|| machine_reference.clone(), str::to_owned);
            let settings_file = default_settings_file(&machine_name)
                .filter(|settings_file| settings_file.is_file());
            let settings = settings_file
                .as_deref()
                .and_then(readable_text)
                .map(|settings_text| parsed_machine_settings(&settings_text))
                .unwrap_or_default();
            Some(VirtualBoxMachine {
                pid: scanned.pid,
                frontend: if program_stem(&scanned.command_line) == Some("VBoxHeadless") {
                    VirtualBoxFrontend::Headless
                } else {
                    VirtualBoxFrontend::Windowed
                },
                guest_memory: settings.guest_memory,
                guest_cores: settings.guest_cores,
                settings_file,
                started_at: scanned.started_at,
                memory_in_use: ByteSize::b(
                    process_scan
                        .process_table
                        .tree_totals(scanned.pid)
                        .resident_bytes,
                ),
                machine_name,
                machine_reference,
            })
        })
        .collect();
    machines.sort_by(|first, second| first.machine_name.cmp(&second.machine_name));
    Ok(VirtualBoxMachines { machines })
}

#[must_use]
pub fn settings_folder_of(machine: &VirtualBoxMachine) -> Option<&Path> {
    machine.settings_file.as_deref()?.parent()
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;

    use super::{MachineSettings, option_value, parsed_machine_settings};

    #[test]
    fn a_headless_launch_names_its_machine_by_comment_and_uuid() {
        let command_line: Vec<String> = [
            "/run/wrappers/bin/VBoxHeadless",
            "--comment",
            "windows11",
            "--startvm",
            "6c1f1d2e-4b7a-4e8f-9a3b-0d5c2e7f8a91",
            "--vrde",
            "config",
        ]
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect();
        assert_eq!(
            option_value(&command_line, &["--comment"]),
            Some("windows11")
        );
        assert_eq!(
            option_value(&command_line, &["--startvm"]),
            Some("6c1f1d2e-4b7a-4e8f-9a3b-0d5c2e7f8a91")
        );
    }

    #[test]
    fn the_settings_file_gives_memory_and_cores() {
        let settings_text = r#"<Machine uuid="{6c1f}" name="windows11"><Hardware><CPU count="4"><PAE enabled="true"/></CPU><Memory RAMSize="8192"/></Hardware></Machine>"#;
        assert_eq!(
            parsed_machine_settings(settings_text),
            MachineSettings {
                guest_memory: Some(ByteSize::gib(8)),
                guest_cores: Some(4),
            }
        );
    }
}
