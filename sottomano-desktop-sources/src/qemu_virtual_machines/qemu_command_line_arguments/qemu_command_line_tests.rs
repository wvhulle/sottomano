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
    let relative_socket = launched_with(&["-qmp", "unix:qmp.sock,server=on,wait=off"]).qmp_socket;
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
    let program_of =
        |arguments: &[&str]| LaunchedProgram::from(command_line_of(arguments).as_slice());
    assert_eq!(
        program_of(&["/usr/bin/qemu-system-x86_64"]),
        LaunchedProgram::QemuEmulator
    );
    assert_eq!(program_of(&["qemu-kvm"]), LaunchedProgram::QemuEmulator);
    assert_eq!(
        program_of(&["qemu-img", "info", "disk.qcow2"]),
        LaunchedProgram::OtherProgram
    );
    assert_eq!(program_of(&[]), LaunchedProgram::OtherProgram);
}
