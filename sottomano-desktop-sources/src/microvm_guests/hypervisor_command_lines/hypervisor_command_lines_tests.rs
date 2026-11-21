use std::path::PathBuf;

use bytesize::ByteSize;

use super::{
    GuestKernelImage, HypervisorApiSocket, HypervisorMachineName, MicrovmHypervisor,
    launched_hypervisor, parsed_launch, with_firecracker_config,
};
use crate::guest_machine_sizing::{GuestCoreCount, GuestMemorySize};

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
    assert_eq!(
        launch.guest_memory,
        GuestMemorySize::Configured(ByteSize::gib(1))
    );
    assert_eq!(launch.guest_cores, GuestCoreCount::Configured(2));
    assert_eq!(
        launch.api_socket,
        HypervisorApiSocket::Listening(PathBuf::from(
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
    assert_eq!(
        launch.named_machine,
        HypervisorMachineName::Named("sandbox".to_owned())
    );
    assert_eq!(
        launch.guest_memory,
        GuestMemorySize::Configured(ByteSize::mib(256))
    );
    assert_eq!(launch.guest_cores, GuestCoreCount::Configured(2));
    assert_eq!(
        launch.kernel,
        GuestKernelImage::Booted(PathBuf::from("/srv/vmlinux"))
    );
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
    assert_eq!(
        crosvm.guest_memory,
        GuestMemorySize::Configured(ByteSize::gib(2))
    );
    assert_eq!(crosvm.guest_cores, GuestCoreCount::Configured(4));
    assert_eq!(
        crosvm.kernel,
        GuestKernelImage::Booted(PathBuf::from("/boot/vmlinux"))
    );
    let kvmtool = parsed_launch(
        MicrovmHypervisor::Kvmtool,
        &arguments(&["lkvm", "run", "-m", "512", "-c", "1", "--name", "scratch"]),
    );
    assert_eq!(
        kvmtool.guest_memory,
        GuestMemorySize::Configured(ByteSize::mib(512))
    );
    assert_eq!(
        kvmtool.named_machine,
        HypervisorMachineName::Named("scratch".to_owned())
    );
}
