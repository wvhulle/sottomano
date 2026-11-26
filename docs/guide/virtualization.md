# Virtualization

Bottom panel, right. One row per machine or container; groups for kinds this computer does not run stay hidden.

| Row control | Meaning |
|---|---|
| Switch | the shell can start and stop it; stopped guests are listed too |
| Stop square | the shell can stop it but not start it |
| Muted switch | it runs, but the shell cannot control it |

<div class="beside">
<div class="beside-text">

Starting a stopped Podman container, then hovering an Incus instance and a Docker container.

</div>

<img src="https://sottomano.work/recordings/tour_virtualization.webp" srcset="https://sottomano.work/recordings/tour_virtualization.webp 2x" alt="Opening the Virtualization menu, starting a Podman container, then hovering an Incus instance and a Docker container">

</div>

## Groups

| Group | Press | Popup |
|---|---|---|
| QEMU machines | libvirt: switch; QMP socket: stop square; right press: focus its window | cores, memory, disks, ports |
| VirtualBox machines | switch | memory, cores, settings file |
| VMware machines | stop square | memory, guest system, configuration file |
| microvm.nix guests | `microvm@` service: switch; Cloud Hypervisor or Firecracker API socket: stop square | hypervisor, memory, service |
| Incus instances | switch | kind, status, project, addresses, image |
| nspawn containers | switch | service, addresses, root directory |
| LXC containers | switch | state, addresses, config file |
| Podman containers | switch | image, CPU, memory, engine |
| Docker containers | switch | image, CPU, memory, engine |
| Apptainer containers | stop square | image, command, runtime |
| Waydroid | switch on the container and on the session | container unit, session state |
| Flatpak apps | stop square | app, runtime, sandbox |
| Memory limits | open `systemd-cgtop` for that cgroup | throttle mark, cap, swap, kills |

## Permissions

| Group | Needs |
|---|---|
| microvm.nix guests, nspawn containers, Waydroid container | `programs.sottomano.containerControl.enable` and your user in `programs.sottomano.groups.containerControl` |
| Incus instances | your user in `incus-admin` or `lxd` |
| Podman and Docker containers | access to the engine's socket |
| LXC containers | a container your user may manage |

A press the guest's manager refuses is written to the shell's log, and the row keeps its state.

<div class="beside">
<div class="beside-text">

The Waydroid session.

</div>

<img src="https://sottomano.work/recordings/waydroid_container_session.webp" srcset="https://sottomano.work/recordings/waydroid_container_session.webp 2x" alt="Opening the Waydroid group from the Virtualization menu and hovering its session">

</div>

<div class="beside">
<div class="beside-text">

The CI runners slice against its memory cap.

</div>

<img src="https://sottomano.work/recordings/memory_limits_slice.webp" srcset="https://sottomano.work/recordings/memory_limits_slice.webp 2x" alt="Opening the memory limits group from the Virtualization menu and hovering the CI runners slice">

</div>
