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

#[must_use]
pub fn is_qemu_command_line(command_line: &[String]) -> bool {
    command_line
        .first()
        .and_then(|executable| Path::new(executable).file_name())
        .and_then(|file_name| file_name.to_str())
        .is_some_and(|file_name| {
            QEMU_EXECUTABLE_PREFIXES
                .iter()
                .any(|prefix| file_name.starts_with(prefix))
        })
}

fn normalized_flag(argument: &str) -> Option<&str> {
    argument
        .strip_prefix("--")
        .or_else(|| argument.strip_prefix('-'))
}

fn option_values<'a>(command_line: &'a [String], flag: &'a str) -> impl Iterator<Item = &'a str> {
    command_line.windows(2).filter_map(move |pair| {
        (normalized_flag(&pair[0]) == Some(flag)).then_some(pair[1].as_str())
    })
}

fn has_flag(command_line: &[String], flag: &str) -> bool {
    command_line
        .iter()
        .any(|argument| normalized_flag(argument) == Some(flag))
}

fn suboption<'a>(option_value: &'a str, key: &str) -> Option<&'a str> {
    option_value
        .split(',')
        .find_map(|segment| segment.strip_prefix(key)?.strip_prefix('='))
}

fn suboptions<'a>(option_value: &'a str, key: &'a str) -> impl Iterator<Item = &'a str> {
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

fn machine_name_of(command_line: &[String]) -> String {
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
                .map(|file_name| file_name.to_string_lossy().into_owned())
        })
        .unwrap_or_default()
}

fn guest_cores_of(command_line: &[String]) -> u32 {
    option_values(command_line, "smp")
        .last()
        .and_then(|smp_value| {
            leading_segment(smp_value)
                .and_then(|count| count.parse().ok())
                .or_else(|| suboption(smp_value, "cpus").and_then(|count| count.parse().ok()))
                .or_else(|| {
                    let topology_counts: Vec<u32> =
                        ["sockets", "dies", "clusters", "cores", "threads"]
                            .iter()
                            .filter_map(|key| suboption(smp_value, key)?.parse().ok())
                            .collect();
                    (!topology_counts.is_empty()).then(|| topology_counts.iter().product())
                })
        })
        .unwrap_or(DEFAULT_GUEST_CORES)
}

fn memory_size_of(size_text: &str) -> Option<ByteSize> {
    let digits_end = size_text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(size_text.len());
    let amount: u64 = size_text[..digits_end].parse().ok()?;
    let multiplier = match size_text[digits_end..].to_ascii_lowercase().as_str() {
        "" | "m" | "mb" | "mib" => 1024 * 1024,
        "k" | "kb" | "kib" => 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        "t" | "tb" | "tib" => 1024 * 1024 * 1024 * 1024,
        "b" => 1,
        _ => return None,
    };
    Some(ByteSize::b(amount * multiplier))
}

fn guest_memory_of(command_line: &[String]) -> ByteSize {
    option_values(command_line, "m")
        .last()
        .and_then(|memory_value| {
            suboption(memory_value, "size").or_else(|| leading_segment(memory_value))
        })
        .and_then(memory_size_of)
        .unwrap_or(DEFAULT_GUEST_MEMORY)
}

fn disk_images_of(command_line: &[String]) -> Vec<PathBuf> {
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
    address_with_port.rsplit(':').next()?.parse().ok()
}

fn forwarded_port_of(hostfwd_rule: &str) -> Option<ForwardedGuestPort> {
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

fn forwarded_ports_of(command_line: &[String]) -> Vec<ForwardedGuestPort> {
    ["netdev", "nic", "net"]
        .into_iter()
        .flat_map(|flag| option_values(command_line, flag))
        .flat_map(|network_value| suboptions(network_value, "hostfwd"))
        .filter_map(forwarded_port_of)
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

fn qmp_socket_of(command_line: &[String]) -> QmpSocketAccess {
    option_values(command_line, "qmp")
        .chain(option_values(command_line, "qmp-pretty"))
        .find_map(unix_socket_path)
        .or_else(|| control_monitor_chardev_path(command_line))
        .map_or(QmpSocketAccess::NotConfigured, QmpSocketAccess::UnixSocket)
}

fn display_of(command_line: &[String]) -> GuestDisplay {
    let display_disabled = option_values(command_line, "display").any(|display_value| {
        matches!(
            leading_segment(display_value),
            Some("none" | "egl-headless" | "vnc" | "spice-app" | "dbus")
        )
    });
    let remote_only = has_flag(command_line, "vnc") && !has_flag(command_line, "display");
    if has_flag(command_line, "nographic") || display_disabled || remote_only {
        GuestDisplay::Headless
    } else {
        GuestDisplay::Windowed
    }
}

fn nixos_system_of(command_line: &[String]) -> NixosSystemClosure {
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
            machine_name: machine_name_of(command_line),
            guest_cores: guest_cores_of(command_line),
            guest_memory: guest_memory_of(command_line),
            disk_images: disk_images_of(command_line),
            forwarded_ports: forwarded_ports_of(command_line),
            qmp_socket: qmp_socket_of(command_line),
            display: display_of(command_line),
            nixos_system: nixos_system_of(command_line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launched_with(arguments: &[&str]) -> QemuLaunchArguments {
        let command_line: Vec<String> = ["qemu-system-x86_64"]
            .iter()
            .chain(arguments)
            .map(|argument| (*argument).to_owned())
            .collect();
        QemuLaunchArguments::parsed(&command_line)
    }

    #[test]
    fn name_cores_and_memory_come_from_their_flags() {
        let launch = launched_with(&["-name", "test-vm", "-smp", "2", "-m", "3072"]);
        assert_eq!(launch.machine_name, "test-vm");
        assert_eq!(launch.guest_cores, 2);
        assert_eq!(launch.guest_memory, ByteSize::mib(3072));
    }

    #[test]
    fn libvirt_style_suboptions_are_understood() {
        let launch = launched_with(&[
            "-name",
            "guest=windows11,debug-threads=on",
            "-smp",
            "4,sockets=1,cores=4,threads=1",
            "-m",
            "size=8388608k",
        ]);
        assert_eq!(launch.machine_name, "windows11");
        assert_eq!(launch.guest_cores, 4);
        assert_eq!(launch.guest_memory, ByteSize::gib(8));
    }

    #[test]
    fn a_topology_without_a_count_multiplies_out() {
        assert_eq!(launched_with(&["-smp", "sockets=2,cores=3"]).guest_cores, 6);
    }

    #[test]
    fn a_bare_invocation_falls_back_to_qemu_defaults() {
        let launch = launched_with(&[]);
        assert_eq!(launch.machine_name, "qemu-system-x86_64");
        assert_eq!(launch.guest_cores, DEFAULT_GUEST_CORES);
        assert_eq!(launch.guest_memory, DEFAULT_GUEST_MEMORY);
        assert_eq!(launch.display, GuestDisplay::Windowed);
        assert_eq!(launch.qmp_socket, QmpSocketAccess::NotConfigured);
        assert_eq!(launch.nixos_system, NixosSystemClosure::NotNixos);
    }

    #[test]
    fn disk_images_come_from_drives_legacy_flags_and_blockdevs() {
        let launch = launched_with(&[
            "-drive",
            "cache=writeback,file=first.qcow2,if=none",
            "-hda",
            "second.img",
            "-blockdev",
            r#"{"driver":"qcow2","file":{"driver":"file","filename":"third.qcow2"}}"#,
        ]);
        assert_eq!(
            launch.disk_images,
            ["first.qcow2", "second.img", "third.qcow2"].map(PathBuf::from)
        );
    }

    #[test]
    fn host_forwards_list_protocol_and_both_ports() {
        let launch = launched_with(&[
            "-netdev",
            "user,id=user.0,hostfwd=tcp::2222-:22,hostfwd=udp:127.0.0.1:5353-:53",
        ]);
        assert_eq!(
            launch.forwarded_ports,
            [
                ForwardedGuestPort {
                    protocol: "tcp".to_owned(),
                    host_port: 2222,
                    guest_port: 22,
                },
                ForwardedGuestPort {
                    protocol: "udp".to_owned(),
                    host_port: 5353,
                    guest_port: 53,
                },
            ]
        );
    }

    #[test]
    fn a_qmp_socket_is_found_directly_or_through_a_control_monitor() {
        assert_eq!(
            launched_with(&["-qmp", "unix:/tmp/direct.qmp,server=on,wait=off"]).qmp_socket,
            QmpSocketAccess::UnixSocket(PathBuf::from("/tmp/direct.qmp"))
        );
        assert_eq!(
            launched_with(&[
                "-chardev",
                "socket,id=qmp0,path=/tmp/monitor.qmp,server=on,wait=off",
                "-mon",
                "chardev=qmp0,mode=control",
            ])
            .qmp_socket,
            QmpSocketAccess::UnixSocket(PathBuf::from("/tmp/monitor.qmp"))
        );
        assert_eq!(
            launched_with(&[
                "-chardev",
                "socket,id=charmonitor,fd=31",
                "-mon",
                "chardev=charmonitor,mode=control"
            ])
            .qmp_socket,
            QmpSocketAccess::NotConfigured
        );
    }

    #[test]
    fn a_vm_without_a_local_window_is_headless() {
        for headless_arguments in [
            &["-nographic"][..],
            &["-display", "none"],
            &["-display", "none", "-spice", "port=5900"],
            &["-vnc", ":1"],
        ] {
            assert_eq!(
                launched_with(headless_arguments).display,
                GuestDisplay::Headless,
                "{headless_arguments:?}"
            );
        }
        assert_eq!(
            launched_with(&["-display", "gtk"]).display,
            GuestDisplay::Windowed
        );
    }

    #[test]
    fn a_nixos_system_is_named_after_its_store_path() {
        let launch = launched_with(&[
            "-kernel",
            "/nix/store/00000000000000000000000000000000-nixos-system-test-vm-26.11/kernel",
        ]);
        assert_eq!(
            launch.nixos_system,
            NixosSystemClosure::Named("test-vm-26.11".to_owned())
        );
    }

    #[test]
    fn a_relative_qmp_socket_lives_in_the_qemu_working_directory() {
        let relative_socket =
            launched_with(&["-qmp", "unix:qmp.sock,server=on,wait=off"]).qmp_socket;
        assert_eq!(relative_socket.connectable_path(), None);
        let resolved_socket = relative_socket.resolved_in(Path::new("/tmp/nix-vm.Ab12Cd34Ef"));
        assert_eq!(
            resolved_socket.connectable_path(),
            Some(Path::new("/tmp/nix-vm.Ab12Cd34Ef/qmp.sock"))
        );
        assert_eq!(
            resolved_socket.clone().resolved_in(Path::new("/elsewhere")),
            resolved_socket
        );
    }

    #[test]
    fn only_qemu_system_executables_are_virtual_machines() {
        let command_line_of = |arguments: &[&str]| -> Vec<String> {
            arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect()
        };
        assert!(is_qemu_command_line(&command_line_of(&[
            "/usr/bin/qemu-system-x86_64"
        ])));
        assert!(is_qemu_command_line(&command_line_of(&["qemu-kvm"])));
        assert!(!is_qemu_command_line(&command_line_of(&[
            "qemu-img",
            "info",
            "disk.qcow2"
        ])));
        assert!(!is_qemu_command_line(&[]));
    }
}
