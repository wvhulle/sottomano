#[cfg(test)]
mod qemu_command_line_tests;

use std::path::{Path, PathBuf};

use bytesize::ByteSize;
use serde::Deserialize;

const DEFAULT_GUEST_CORES: u32 = 1;
const DEFAULT_GUEST_MEMORY: ByteSize = ByteSize::mib(128);
const NIXOS_SYSTEM_STORE_MARKER: &str = "-nixos-system-";
const QEMU_EXECUTABLE_PREFIXES: [&str; 2] = ["qemu-system-", "qemu-kvm"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestDisplay {
    Windowed,
    Headless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NixosSystemClosure {
    Named(String),
    NotNixos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QmpSocketAccess {
    UnixSocket(PathBuf),
    NotConfigured,
}

impl QmpSocketAccess {
    #[must_use]
    pub(crate) fn resolved_in(self, working_directory: &Path) -> Self {
        match self {
            QmpSocketAccess::UnixSocket(socket_path) if socket_path.is_relative() => {
                QmpSocketAccess::UnixSocket(working_directory.join(socket_path))
            }
            unchanged => unchanged,
        }
    }

    #[must_use]
    pub(crate) fn connectable_path(&self) -> Option<&Path> {
        match self {
            QmpSocketAccess::UnixSocket(socket_path) if socket_path.is_absolute() => {
                Some(socket_path)
            }
            QmpSocketAccess::UnixSocket(_) | QmpSocketAccess::NotConfigured => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedGuestPort {
    pub protocol: String,
    pub host_port: u16,
    pub guest_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QemuLaunchArguments {
    pub machine_name: String,
    pub guest_cores: u32,
    pub guest_memory: ByteSize,
    pub disk_images: Vec<PathBuf>,
    pub forwarded_ports: Vec<ForwardedGuestPort>,
    pub qmp_socket: QmpSocketAccess,
    pub display: GuestDisplay,
    pub nixos_system: NixosSystemClosure,
}

#[derive(Deserialize)]
struct BlockdevFileNode {
    filename: Option<PathBuf>,
    file: Option<Box<BlockdevFileNode>>,
}

impl BlockdevFileNode {
    fn innermost_filename(self) -> Option<PathBuf> {
        self.filename
            .or_else(|| self.file.and_then(|inner| inner.innermost_filename()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchedProgram {
    QemuEmulator,
    OtherProgram,
}

impl From<&[String]> for LaunchedProgram {
    fn from(command_line: &[String]) -> Self {
        if command_line
            .first()
            .and_then(|executable| Path::new(executable).file_name())
            .and_then(|file_name| file_name.to_str())
            .is_some_and(|file_name| {
                QEMU_EXECUTABLE_PREFIXES
                    .iter()
                    .any(|prefix| file_name.starts_with(prefix))
            })
        {
            LaunchedProgram::QemuEmulator
        } else {
            LaunchedProgram::OtherProgram
        }
    }
}

fn normalized_flag(argument: &str) -> Option<&str> {
    argument
        .strip_prefix("--")
        .or_else(|| argument.strip_prefix('-'))
}

fn option_values<'arguments>(
    command_line: &'arguments [String],
    flag: &'arguments str,
) -> impl Iterator<Item = &'arguments str> {
    command_line.windows(2).filter_map(move |pair| {
        (normalized_flag(&pair[0]) == Some(flag)).then_some(pair[1].as_str())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlagPresence {
    Given,
    Absent,
}

fn flag_presence(command_line: &[String], flag: &str) -> FlagPresence {
    if command_line
        .iter()
        .any(|argument| normalized_flag(argument) == Some(flag))
    {
        FlagPresence::Given
    } else {
        FlagPresence::Absent
    }
}

fn suboption<'option>(option_value: &'option str, key: &str) -> Option<&'option str> {
    option_value
        .split(',')
        .find_map(|segment| segment.strip_prefix(key)?.strip_prefix('='))
}

fn suboptions<'option>(
    option_value: &'option str,
    key: &'option str,
) -> impl Iterator<Item = &'option str> {
    option_value
        .split(',')
        .filter_map(move |segment| segment.strip_prefix(key)?.strip_prefix('='))
}

fn leading_segment(option_value: &str) -> Option<&str> {
    option_value
        .split(',')
        .next()
        .filter(|segment| !segment.contains('='))
}

fn launched_machine_name(command_line: &[String]) -> String {
    option_values(command_line, "name")
        .last()
        .and_then(|name_value| {
            suboption(name_value, "guest").or_else(|| leading_segment(name_value))
        })
        .map(str::to_owned)
        .or_else(|| {
            command_line
                .first()
                .and_then(|executable| Path::new(executable).file_name())
                .map(|file_name| file_name.display().to_string())
        })
        .unwrap_or_default()
}

fn smp_guest_cores(command_line: &[String]) -> u32 {
    option_values(command_line, "smp")
        .last()
        .and_then(|smp_value| {
            let parse_core_count = |count_text: &str| {
                count_text
                    .parse::<u32>()
                    .inspect_err(|parse_failure| {
                        tracing::warn!(
                            "qemu command line: invalid count {count_text:?} in -smp \
                             {smp_value:?}: {parse_failure}"
                        );
                    })
                    .ok()
            };
            leading_segment(smp_value)
                .and_then(parse_core_count)
                .or_else(|| suboption(smp_value, "cpus").and_then(parse_core_count))
                .or_else(|| {
                    let topology_counts: Vec<u32> =
                        ["sockets", "dies", "clusters", "cores", "threads"]
                            .iter()
                            .filter_map(|key| parse_core_count(suboption(smp_value, key)?))
                            .collect();
                    (!topology_counts.is_empty()).then(|| topology_counts.iter().product())
                })
        })
        .unwrap_or(DEFAULT_GUEST_CORES)
}

fn parsed_memory_size(size_text: &str) -> Option<ByteSize> {
    let digits_end = size_text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(size_text.len());
    let amount: u64 = size_text[..digits_end]
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!("qemu command line: invalid memory size {size_text:?}: {parse_failure}");
        })
        .ok()?;
    let multiplier = match size_text[digits_end..].to_ascii_lowercase().as_str() {
        "" | "m" | "mb" | "mib" => 1024 * 1024,
        "k" | "kb" | "kib" => 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        "t" | "tb" | "tib" => 1024 * 1024 * 1024 * 1024,
        "b" => 1,
        unknown_unit => {
            tracing::warn!(
                "qemu command line: unknown memory unit {unknown_unit:?} in {size_text:?}"
            );
            return None;
        }
    };
    Some(ByteSize::b(amount * multiplier))
}

fn configured_guest_memory(command_line: &[String]) -> ByteSize {
    option_values(command_line, "m")
        .last()
        .and_then(|memory_value| {
            suboption(memory_value, "size").or_else(|| leading_segment(memory_value))
        })
        .and_then(parsed_memory_size)
        .unwrap_or(DEFAULT_GUEST_MEMORY)
}

fn attached_disk_images(command_line: &[String]) -> Vec<PathBuf> {
    let drive_files = option_values(command_line, "drive")
        .filter_map(|drive_value| suboption(drive_value, "file"))
        .map(PathBuf::from);
    let legacy_disks = ["hda", "hdb", "hdc", "hdd"]
        .into_iter()
        .flat_map(|flag| option_values(command_line, flag))
        .map(PathBuf::from);
    let blockdev_files = option_values(command_line, "blockdev").filter_map(|blockdev_value| {
        if blockdev_value.starts_with('{') {
            serde_json::from_str::<BlockdevFileNode>(blockdev_value)
                .inspect_err(|json_failure| {
                    tracing::warn!(
                        "qemu command line: invalid -blockdev json {blockdev_value:?}: \
                         {json_failure}"
                    );
                })
                .ok()?
                .innermost_filename()
        } else {
            suboption(blockdev_value, "filename").map(PathBuf::from)
        }
    });
    let mut disk_images: Vec<PathBuf> = drive_files
        .chain(legacy_disks)
        .chain(blockdev_files)
        .collect();
    disk_images.dedup();
    disk_images
}

fn port_after_last_colon(address_with_port: &str) -> Option<u16> {
    let port_text = address_with_port.rsplit(':').next()?;
    port_text
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "qemu command line: invalid port {port_text:?} in {address_with_port:?}: \
                 {parse_failure}"
            );
        })
        .ok()
}

fn parsed_forwarded_port(hostfwd_rule: &str) -> Option<ForwardedGuestPort> {
    let (protocol, addresses) = hostfwd_rule.split_once(':')?;
    let (host_side, guest_side) = addresses.split_once('-')?;
    Some(ForwardedGuestPort {
        protocol: if protocol.is_empty() {
            "tcp".to_owned()
        } else {
            protocol.to_owned()
        },
        host_port: port_after_last_colon(host_side)?,
        guest_port: port_after_last_colon(guest_side)?,
    })
}

fn host_forwarded_ports(command_line: &[String]) -> Vec<ForwardedGuestPort> {
    ["netdev", "nic", "net"]
        .into_iter()
        .flat_map(|flag| option_values(command_line, flag))
        .flat_map(|network_value| suboptions(network_value, "hostfwd"))
        .filter_map(parsed_forwarded_port)
        .collect()
}

fn unix_socket_path(chardev_address: &str) -> Option<PathBuf> {
    let socket_path = chardev_address.strip_prefix("unix:")?;
    socket_path
        .split(',')
        .next()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

fn control_monitor_chardev_path(command_line: &[String]) -> Option<PathBuf> {
    let control_chardev_ids: Vec<&str> = option_values(command_line, "mon")
        .filter(|monitor_value| suboption(monitor_value, "mode") == Some("control"))
        .filter_map(|monitor_value| suboption(monitor_value, "chardev"))
        .collect();
    option_values(command_line, "chardev").find_map(|chardev_value| {
        let is_control_socket = chardev_value.starts_with("socket,")
            && suboption(chardev_value, "id")
                .is_some_and(|chardev_id| control_chardev_ids.contains(&chardev_id));
        is_control_socket
            .then(|| suboption(chardev_value, "path"))
            .flatten()
            .map(PathBuf::from)
    })
}

fn qmp_socket_access(command_line: &[String]) -> QmpSocketAccess {
    option_values(command_line, "qmp")
        .chain(option_values(command_line, "qmp-pretty"))
        .find_map(unix_socket_path)
        .or_else(|| control_monitor_chardev_path(command_line))
        .map_or(QmpSocketAccess::NotConfigured, QmpSocketAccess::UnixSocket)
}

fn guest_display_mode(command_line: &[String]) -> GuestDisplay {
    let display_disabled = option_values(command_line, "display").any(|display_value| {
        matches!(
            leading_segment(display_value),
            Some("none" | "egl-headless" | "vnc" | "spice-app" | "dbus")
        )
    });
    let remote_only = flag_presence(command_line, "vnc") == FlagPresence::Given
        && flag_presence(command_line, "display") == FlagPresence::Absent;
    if flag_presence(command_line, "nographic") == FlagPresence::Given
        || display_disabled
        || remote_only
    {
        GuestDisplay::Headless
    } else {
        GuestDisplay::Windowed
    }
}

fn nixos_system_closure(command_line: &[String]) -> NixosSystemClosure {
    command_line
        .iter()
        .find_map(|argument| {
            let marker_start = argument.find(NIXOS_SYSTEM_STORE_MARKER)?;
            let after_marker = &argument[marker_start + NIXOS_SYSTEM_STORE_MARKER.len()..];
            let name_end = after_marker
                .find(|character: char| character == '/' || character.is_whitespace())
                .unwrap_or(after_marker.len());
            let system_name = &after_marker[..name_end];
            (!system_name.is_empty()).then(|| system_name.to_owned())
        })
        .map_or(NixosSystemClosure::NotNixos, NixosSystemClosure::Named)
}

impl QemuLaunchArguments {
    #[must_use]
    pub fn parsed(command_line: &[String]) -> Self {
        Self {
            machine_name: launched_machine_name(command_line),
            guest_cores: smp_guest_cores(command_line),
            guest_memory: configured_guest_memory(command_line),
            disk_images: attached_disk_images(command_line),
            forwarded_ports: host_forwarded_ports(command_line),
            qmp_socket: qmp_socket_access(command_line),
            display: guest_display_mode(command_line),
            nixos_system: nixos_system_closure(command_line),
        }
    }
}
