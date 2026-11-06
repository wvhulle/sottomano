use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    qemu_virtual_machines::{
        qemu_command_line_arguments::{
            ForwardedGuestPort, GuestDisplay, NixosSystemClosure, QemuLaunchArguments,
            QmpSocketAccess,
        },
        qmp_monitor_events::{GuestRunState, QmpPowerdownSender},
        running_virtual_machines::{
            GuestPowerControl, RunningVirtualMachine, VirtualMachineOrigin, VirtualMachinesReadout,
        },
    },
    system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad,
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::virtual_machines_group::{VirtualMachinesGroup, VirtualMachinesMessage},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::showcased_group::ShowcasedGroup;

pub const INTERACTIVE_VM_PID: i32 = 4100;

fn interactive_sample_vm(clock: Clock) -> RunningVirtualMachine {
    let qmp_socket = PathBuf::from("/run/user/1000/bootvine-sample-vm-slow-services.qmp");
    RunningVirtualMachine {
        pid: INTERACTIVE_VM_PID,
        origin: VirtualMachineOrigin::SessionUser,
        launch: QemuLaunchArguments {
            machine_name: "sample-vm-slow-services".to_owned(),
            guest_cores: 2,
            guest_memory: ByteSize::gib(2),
            disk_images: vec![PathBuf::from(
                "/home/user/.cache/bootvine-sample-vm-slow-services.qcow2",
            )],
            forwarded_ports: vec![ForwardedGuestPort {
                protocol: "tcp".to_owned(),
                host_port: 2222,
                guest_port: 22,
            }],
            qmp_socket: QmpSocketAccess::UnixSocket(qmp_socket.clone()),
            display: GuestDisplay::Windowed,
            nixos_system: NixosSystemClosure::Named("sample-vm-slow-services-26.11".to_owned()),
        },
        run_state: GuestRunState::Running,
        power_control: GuestPowerControl::QmpSession(QmpPowerdownSender::detached_fixture(
            qmp_socket,
        )),
        started_at: clock.wall - Duration::from_mins(12),
        cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(1.2)),
        resident_memory: ByteSize::mib(1150),
    }
}

fn nixos_test_vm(clock: Clock) -> RunningVirtualMachine {
    RunningVirtualMachine {
        pid: 4200,
        origin: VirtualMachineOrigin::NixosTestBuild,
        launch: QemuLaunchArguments {
            machine_name: "encrypted-volume".to_owned(),
            guest_cores: 2,
            guest_memory: ByteSize::gib(1),
            disk_images: vec![PathBuf::from("/build/vm-state-machine/machine.qcow2")],
            forwarded_ports: Vec::new(),
            qmp_socket: QmpSocketAccess::UnixSocket(PathBuf::from("/build/vm-state-machine/qmp")),
            display: GuestDisplay::Headless,
            nixos_system: NixosSystemClosure::Named("machine-test".to_owned()),
        },
        run_state: GuestRunState::Unreported,
        power_control: GuestPowerControl::Unavailable,
        started_at: clock.wall - Duration::from_secs(40),
        cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(5.4)),
        resident_memory: ByteSize::mib(760),
    }
}

fn paused_libvirt_domain(clock: Clock) -> RunningVirtualMachine {
    RunningVirtualMachine {
        pid: 4300,
        origin: VirtualMachineOrigin::LibvirtDomain,
        launch: QemuLaunchArguments {
            machine_name: "windows11".to_owned(),
            guest_cores: 4,
            guest_memory: ByteSize::gib(8),
            disk_images: vec![PathBuf::from("/var/lib/libvirt/images/windows11.qcow2")],
            forwarded_ports: Vec::new(),
            qmp_socket: QmpSocketAccess::NotConfigured,
            display: GuestDisplay::Headless,
            nixos_system: NixosSystemClosure::NotNixos,
        },
        run_state: GuestRunState::Paused,
        power_control: GuestPowerControl::LibvirtDomain("windows11".to_owned()),
        started_at: clock.wall - Duration::from_hours(3),
        cpu_load: ProcessTreeCpuLoad::BusyShare(Ratio::new::<percent>(0.0)),
        resident_memory: ByteSize::gib(6),
    }
}

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = VirtualMachinesGroup::default();
    let _ = group.update(
        VirtualMachinesMessage::Machines(SnapshotDelivery::fixture_delivered(
            Ok(VirtualMachinesReadout {
                machines: vec![
                    paused_libvirt_domain(clock),
                    interactive_sample_vm(clock),
                    nixos_test_vm(clock),
                ],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::VirtualMachines(Box::new(group))
}
