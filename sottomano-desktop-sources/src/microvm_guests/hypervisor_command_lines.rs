use std::path::PathBuf;

use bytesize::ByteSize;
use serde::Deserialize;

use crate::system_protocol_clients::procfs_process_scan::program_stem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MicrovmHypervisor {
    CloudHypervisor,
    Firecracker,
    Crosvm,
    Kvmtool,
}

impl MicrovmHypervisor {
    #[must_use]
    pub const fn product_name(self) -> &'static str {
        match self {
            MicrovmHypervisor::CloudHypervisor => "Cloud Hypervisor",
            MicrovmHypervisor::Firecracker => "Firecracker",
            MicrovmHypervisor::Crosvm => "crosvm",
            MicrovmHypervisor::Kvmtool => "kvmtool",
        }
    }

    #[must_use]
    pub const fn has_power_button_api(self) -> bool {
        matches!(
            self,
            MicrovmHypervisor::CloudHypervisor | MicrovmHypervisor::Firecracker
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HypervisorLaunch {
    pub named_machine: Option<String>,
    pub guest_memory: Option<ByteSize>,
    pub guest_cores: Option<u32>,
    pub kernel: Option<PathBuf>,
    pub api_socket: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
}

#[must_use]
pub fn launched_hypervisor(command_line: &[String]) -> Option<MicrovmHypervisor> {
    let runs_a_guest = || {
        command_line
            .get(1)
            .is_some_and(|subcommand| subcommand == "run")
    };
    match program_stem(command_line)? {
        "cloud-hypervisor" => Some(MicrovmHypervisor::CloudHypervisor),
        "firecracker" => Some(MicrovmHypervisor::Firecracker),
        "crosvm" if runs_a_guest() => Some(MicrovmHypervisor::Crosvm),
        "lkvm" | "kvmtool" if runs_a_guest() => Some(MicrovmHypervisor::Kvmtool),
        _ => None,
    }
}

fn option_values<'a>(command_line: &'a [String], option_names: &[&str]) -> Vec<&'a str> {
    let mut found_values = Vec::new();
    let mut arguments = command_line.iter().skip(1);
    while let Some(argument) = arguments.next() {
        for option_name in option_names {
            if argument == option_name {
                if let Some(option_value) = arguments.next() {
                    found_values.push(option_value.as_str());
                }
            } else if let Some(option_value) = argument
                .strip_prefix(option_name)
                .and_then(|after_name| after_name.strip_prefix('='))
            {
                found_values.push(option_value);
            }
        }
    }
    found_values
}

fn option_value<'a>(command_line: &'a [String], option_names: &[&str]) -> Option<&'a str> {
    option_values(command_line, option_names).into_iter().last()
}

fn keyed_field<'a>(option_value: &'a str, field_key: &str) -> Option<&'a str> {
    option_value.split(',').find_map(|field| {
        field
            .strip_prefix(field_key)
            .and_then(|after_key| after_key.strip_prefix('='))
    })
}

fn parsed_size_with_suffix(size_text: &str) -> Option<ByteSize> {
    let size_text = size_text.trim();
    let (digits, unit_multiplier) = match size_text.chars().last()? {
        'K' | 'k' => (&size_text[..size_text.len() - 1], 1 << 10),
        'M' | 'm' => (&size_text[..size_text.len() - 1], 1 << 20),
        'G' | 'g' => (&size_text[..size_text.len() - 1], 1 << 30),
        'T' | 't' => (&size_text[..size_text.len() - 1], 1_u64 << 40),
        _ => (size_text, 1),
    };
    digits
        .parse::<u64>()
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "microvm command line: invalid memory size {size_text:?}: {parse_failure}"
            );
        })
        .ok()
        .map(|amount| ByteSize::b(amount * unit_multiplier))
}

fn mebibytes(size_text: &str) -> Option<ByteSize> {
    size_text
        .trim()
        .parse::<u64>()
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "microvm command line: invalid memory mebibytes {size_text:?}: {parse_failure}"
            );
        })
        .ok()
        .map(ByteSize::mib)
}

fn parsed_guest_cores(cores_text: &str) -> Option<u32> {
    cores_text
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "microvm command line: invalid core count {cores_text:?}: {parse_failure}"
            );
        })
        .ok()
}

fn cloud_hypervisor_launch(command_line: &[String]) -> HypervisorLaunch {
    let memory_option = option_value(command_line, &["--memory"]);
    let cpus_option = option_value(command_line, &["--cpus"]);
    let api_socket_option = option_value(command_line, &["--api-socket"]);
    HypervisorLaunch {
        named_machine: None,
        guest_memory: memory_option
            .and_then(|memory_option| keyed_field(memory_option, "size"))
            .and_then(parsed_size_with_suffix),
        guest_cores: cpus_option
            .and_then(|cpus_option| keyed_field(cpus_option, "boot"))
            .and_then(parsed_guest_cores),
        kernel: option_value(command_line, &["--kernel"]).map(PathBuf::from),
        api_socket: api_socket_option.map(|socket_option| {
            PathBuf::from(keyed_field(socket_option, "path").unwrap_or(socket_option))
        }),
        config_file: None,
    }
}

#[derive(Deserialize)]
struct FirecrackerMachineConfig {
    vcpu_count: Option<u32>,
    mem_size_mib: Option<u64>,
}

#[derive(Deserialize)]
struct FirecrackerBootSource {
    kernel_image_path: Option<PathBuf>,
}

#[derive(Deserialize)]
struct FirecrackerConfigFile {
    #[serde(rename = "machine-config")]
    machine_config: Option<FirecrackerMachineConfig>,
    #[serde(rename = "boot-source")]
    boot_source: Option<FirecrackerBootSource>,
}

#[must_use]
pub fn with_firecracker_config(
    mut launch: HypervisorLaunch,
    config_text: &str,
) -> HypervisorLaunch {
    let Ok(config_file) = serde_json::from_str::<FirecrackerConfigFile>(config_text) else {
        return launch;
    };
    if let Some(machine_config) = config_file.machine_config {
        launch.guest_memory = machine_config.mem_size_mib.map(ByteSize::mib);
        launch.guest_cores = machine_config.vcpu_count;
    }
    launch.kernel = config_file
        .boot_source
        .and_then(|boot_source| boot_source.kernel_image_path);
    launch
}

fn firecracker_launch(command_line: &[String]) -> HypervisorLaunch {
    HypervisorLaunch {
        named_machine: option_value(command_line, &["--id"]).map(str::to_owned),
        api_socket: option_value(command_line, &["--api-sock"]).map(PathBuf::from),
        config_file: option_value(command_line, &["--config-file"]).map(PathBuf::from),
        ..HypervisorLaunch::default()
    }
}

fn crosvm_launch(command_line: &[String]) -> HypervisorLaunch {
    let memory_option = option_value(command_line, &["--mem", "-m"]);
    let cpus_option = option_value(command_line, &["--cpus", "-c"]);
    HypervisorLaunch {
        guest_memory: memory_option.and_then(|memory_option| {
            mebibytes(keyed_field(memory_option, "size").unwrap_or(memory_option))
        }),
        guest_cores: cpus_option.and_then(|cpus_option| {
            parsed_guest_cores(keyed_field(cpus_option, "num-cores").unwrap_or(cpus_option))
        }),
        kernel: command_line
            .last()
            .filter(|last_argument| !last_argument.starts_with('-'))
            .map(PathBuf::from),
        api_socket: option_value(command_line, &["--socket", "-s"]).map(PathBuf::from),
        ..HypervisorLaunch::default()
    }
}

fn kvmtool_launch(command_line: &[String]) -> HypervisorLaunch {
    HypervisorLaunch {
        named_machine: option_value(command_line, &["--name", "-n"]).map(str::to_owned),
        guest_memory: option_value(command_line, &["--mem", "-m"]).and_then(mebibytes),
        guest_cores: option_value(command_line, &["--cpus", "-c"]).and_then(parsed_guest_cores),
        kernel: option_value(command_line, &["--kernel", "-k"]).map(PathBuf::from),
        ..HypervisorLaunch::default()
    }
}

#[must_use]
pub fn parsed_launch(hypervisor: MicrovmHypervisor, command_line: &[String]) -> HypervisorLaunch {
    match hypervisor {
        MicrovmHypervisor::CloudHypervisor => cloud_hypervisor_launch(command_line),
        MicrovmHypervisor::Firecracker => firecracker_launch(command_line),
        MicrovmHypervisor::Crosvm => crosvm_launch(command_line),
        MicrovmHypervisor::Kvmtool => kvmtool_launch(command_line),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use bytesize::ByteSize;

    use super::{MicrovmHypervisor, launched_hypervisor, parsed_launch, with_firecracker_config};

    fn arguments(command_line: &[&str]) -> Vec<String> {
        command_line
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect()
    }

    #[test]
    fn a_wrapped_nix_program_is_recognised_by_its_stem() {
        assert_eq!(
            launched_hypervisor(&arguments(&[
                "/nix/store/abc-cloud-hypervisor-48.0/bin/.cloud-hypervisor-wrapped",
                "--memory",
                "size=512M",
            ])),
            Some(MicrovmHypervisor::CloudHypervisor)
        );
        assert_eq!(
            launched_hypervisor(&arguments(&["crosvm", "version"])),
            None
        );
        assert_eq!(
            launched_hypervisor(&arguments(&["qemu-system-x86_64"])),
            None
        );
    }

    #[test]
    fn a_microvm_nix_cloud_hypervisor_launch_reads_its_size_cores_and_socket() {
        let launch = parsed_launch(
            MicrovmHypervisor::CloudHypervisor,
            &arguments(&[
                "cloud-hypervisor",
                "--cpus",
                "boot=2",
                "--memory",
                "size=1024M,mergeable=on",
                "--kernel",
                "/nix/store/xyz-linux-6.18/bzImage",
                "--api-socket",
                "path=/var/lib/microvms/ci-builder/ci-builder.sock",
            ]),
        );
        assert_eq!(launch.guest_memory, Some(ByteSize::gib(1)));
        assert_eq!(launch.guest_cores, Some(2));
        assert_eq!(
            launch.api_socket,
            Some(PathBuf::from(
                "/var/lib/microvms/ci-builder/ci-builder.sock"
            ))
        );
    }

    #[test]
    fn a_firecracker_config_file_supplies_size_cores_and_kernel() {
        let launch = parsed_launch(
            MicrovmHypervisor::Firecracker,
            &arguments(&[
                "firecracker",
                "--api-sock",
                "/run/firecracker/sandbox.sock",
                "--config-file",
                "/etc/firecracker/sandbox.json",
                "--id",
                "sandbox",
            ]),
        );
        let launch = with_firecracker_config(
            launch,
            r#"{"machine-config":{"vcpu_count":2,"mem_size_mib":256},"boot-source":{"kernel_image_path":"/srv/vmlinux"}}"#,
        );
        assert_eq!(launch.named_machine.as_deref(), Some("sandbox"));
        assert_eq!(launch.guest_memory, Some(ByteSize::mib(256)));
        assert_eq!(launch.guest_cores, Some(2));
        assert_eq!(launch.kernel, Some(PathBuf::from("/srv/vmlinux")));
    }

    #[test]
    fn crosvm_and_kvmtool_sizes_are_mebibytes() {
        let crosvm = parsed_launch(
            MicrovmHypervisor::Crosvm,
            &arguments(&[
                "crosvm",
                "run",
                "--mem",
                "2048",
                "--cpus",
                "4",
                "/boot/vmlinux",
            ]),
        );
        assert_eq!(crosvm.guest_memory, Some(ByteSize::gib(2)));
        assert_eq!(crosvm.guest_cores, Some(4));
        assert_eq!(crosvm.kernel, Some(PathBuf::from("/boot/vmlinux")));
        let kvmtool = parsed_launch(
            MicrovmHypervisor::Kvmtool,
            &arguments(&["lkvm", "run", "-m", "512", "-c", "1", "--name", "scratch"]),
        );
        assert_eq!(kvmtool.guest_memory, Some(ByteSize::mib(512)));
        assert_eq!(kvmtool.named_machine.as_deref(), Some("scratch"));
    }
}
