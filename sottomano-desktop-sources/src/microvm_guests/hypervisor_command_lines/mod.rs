use std::path::PathBuf;

use bytesize::ByteSize;
use serde::Deserialize;

use crate::{
    guest_machine_sizing::{GuestCoreCount, GuestMemorySize},
    system_protocol_clients::procfs_process_scan::program_stem,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HypervisorMachineName {
    Named(String),
    Unnamed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestKernelImage {
    Booted(PathBuf),
    NotOnCommandLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HypervisorApiSocket {
    Listening(PathBuf),
    NotOpened,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchConfigFile {
    Given(PathBuf),
    CommandLineOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HypervisorLaunch {
    pub named_machine: HypervisorMachineName,
    pub guest_memory: GuestMemorySize,
    pub guest_cores: GuestCoreCount,
    pub kernel: GuestKernelImage,
    pub api_socket: HypervisorApiSocket,
    pub config_file: LaunchConfigFile,
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

fn option_values<'arguments>(
    command_line: &'arguments [String],
    option_names: &[&str],
) -> Vec<&'arguments str> {
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

fn option_value<'arguments>(
    command_line: &'arguments [String],
    option_names: &[&str],
) -> Option<&'arguments str> {
    option_values(command_line, option_names).into_iter().last()
}

fn keyed_field<'option>(option_value: &'option str, field_key: &str) -> Option<&'option str> {
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

fn kernel_option(command_line: &[String], option_names: &[&str]) -> GuestKernelImage {
    option_value(command_line, option_names).map_or(GuestKernelImage::NotOnCommandLine, |kernel| {
        GuestKernelImage::Booted(PathBuf::from(kernel))
    })
}

fn machine_name_option(command_line: &[String], option_names: &[&str]) -> HypervisorMachineName {
    option_value(command_line, option_names).map_or(HypervisorMachineName::Unnamed, |name| {
        HypervisorMachineName::Named(name.to_owned())
    })
}

fn api_socket_option(command_line: &[String], option_names: &[&str]) -> HypervisorApiSocket {
    option_value(command_line, option_names).map_or(HypervisorApiSocket::NotOpened, |socket| {
        HypervisorApiSocket::Listening(PathBuf::from(socket))
    })
}

fn cloud_hypervisor_launch(command_line: &[String]) -> HypervisorLaunch {
    let memory_option = option_value(command_line, &["--memory"]);
    let cpus_option = option_value(command_line, &["--cpus"]);
    let api_socket_option = option_value(command_line, &["--api-socket"]);
    HypervisorLaunch {
        named_machine: HypervisorMachineName::Unnamed,
        guest_memory: memory_option
            .and_then(|memory_option| keyed_field(memory_option, "size"))
            .and_then(parsed_size_with_suffix)
            .map_or(
                GuestMemorySize::HypervisorDefault,
                GuestMemorySize::Configured,
            ),
        guest_cores: cpus_option
            .and_then(|cpus_option| keyed_field(cpus_option, "boot"))
            .and_then(parsed_guest_cores)
            .map_or(
                GuestCoreCount::HypervisorDefault,
                GuestCoreCount::Configured,
            ),
        kernel: kernel_option(command_line, &["--kernel"]),
        api_socket: api_socket_option.map_or(HypervisorApiSocket::NotOpened, |socket_option| {
            HypervisorApiSocket::Listening(PathBuf::from(
                keyed_field(socket_option, "path").unwrap_or(socket_option),
            ))
        }),
        config_file: LaunchConfigFile::CommandLineOnly,
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
struct FirecrackerConfigJson {
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
    let Ok(config_file) = serde_json::from_str::<FirecrackerConfigJson>(config_text) else {
        return launch;
    };
    if let Some(machine_config) = config_file.machine_config {
        launch.guest_memory = machine_config
            .mem_size_mib
            .map_or(GuestMemorySize::HypervisorDefault, |mebibytes| {
                GuestMemorySize::Configured(ByteSize::mib(mebibytes))
            });
        launch.guest_cores = machine_config.vcpu_count.map_or(
            GuestCoreCount::HypervisorDefault,
            GuestCoreCount::Configured,
        );
    }
    launch.kernel = config_file
        .boot_source
        .and_then(|boot_source| boot_source.kernel_image_path)
        .map_or(GuestKernelImage::NotOnCommandLine, GuestKernelImage::Booted);
    launch
}

fn firecracker_launch(command_line: &[String]) -> HypervisorLaunch {
    HypervisorLaunch {
        named_machine: machine_name_option(command_line, &["--id"]),
        guest_memory: GuestMemorySize::HypervisorDefault,
        guest_cores: GuestCoreCount::HypervisorDefault,
        kernel: GuestKernelImage::NotOnCommandLine,
        api_socket: api_socket_option(command_line, &["--api-sock"]),
        config_file: option_value(command_line, &["--config-file"])
            .map_or(LaunchConfigFile::CommandLineOnly, |config_file| {
                LaunchConfigFile::Given(PathBuf::from(config_file))
            }),
    }
}

fn crosvm_launch(command_line: &[String]) -> HypervisorLaunch {
    let memory_option = option_value(command_line, &["--mem", "-m"]);
    let cpus_option = option_value(command_line, &["--cpus", "-c"]);
    HypervisorLaunch {
        named_machine: HypervisorMachineName::Unnamed,
        guest_memory: memory_option
            .and_then(|memory_option| {
                mebibytes(keyed_field(memory_option, "size").unwrap_or(memory_option))
            })
            .map_or(
                GuestMemorySize::HypervisorDefault,
                GuestMemorySize::Configured,
            ),
        guest_cores: cpus_option
            .and_then(|cpus_option| {
                parsed_guest_cores(keyed_field(cpus_option, "num-cores").unwrap_or(cpus_option))
            })
            .map_or(
                GuestCoreCount::HypervisorDefault,
                GuestCoreCount::Configured,
            ),
        kernel: command_line
            .last()
            .filter(|last_argument| !last_argument.starts_with('-'))
            .map_or(GuestKernelImage::NotOnCommandLine, |kernel| {
                GuestKernelImage::Booted(PathBuf::from(kernel))
            }),
        api_socket: api_socket_option(command_line, &["--socket", "-s"]),
        config_file: LaunchConfigFile::CommandLineOnly,
    }
}

fn kvmtool_launch(command_line: &[String]) -> HypervisorLaunch {
    HypervisorLaunch {
        named_machine: machine_name_option(command_line, &["--name", "-n"]),
        guest_memory: option_value(command_line, &["--mem", "-m"])
            .and_then(mebibytes)
            .map_or(
                GuestMemorySize::HypervisorDefault,
                GuestMemorySize::Configured,
            ),
        guest_cores: option_value(command_line, &["--cpus", "-c"])
            .and_then(parsed_guest_cores)
            .map_or(
                GuestCoreCount::HypervisorDefault,
                GuestCoreCount::Configured,
            ),
        kernel: kernel_option(command_line, &["--kernel", "-k"]),
        api_socket: HypervisorApiSocket::NotOpened,
        config_file: LaunchConfigFile::CommandLineOnly,
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
mod hypervisor_command_lines_tests;
