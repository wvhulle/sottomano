# Virtualization

Bottom panel, right. One row per machine or container; groups for kinds this computer does not run stay hidden.

| Row control | Meaning |
|---|---|
| Switch | the shell can start and stop it; stopped guests are listed too |
| Stop square | the shell can stop it but not start it |
| Muted switch | it runs, but the shell cannot control it |

<div class="beside">
<div class="beside-text">

Opening the Virtualization menu and hovering a QEMU machine, an Incus instance and a Docker container.

</div>

<img src="https://sottomano.work/recordings/tour_virtualization.webp" srcset="https://sottomano.work/recordings/tour_virtualization.webp 2x" alt="Opening the Virtualization menu, hovering a QEMU machine and an Incus instance, then a Docker container">

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

A microvm.nix guest running Cloud Hypervisor.

</div>

<img src="https://sottomano.work/recordings/microvm_guests_cloud_hypervisor.webp" srcset="https://sottomano.work/recordings/microvm_guests_cloud_hypervisor.webp 2x" alt="Opening the microvm.nix guests group from the Virtualization menu and hovering a Cloud Hypervisor guest">

</div>

<div class="beside">
<div class="beside-text">

A headless VirtualBox machine.

</div>

<img src="https://sottomano.work/recordings/virtualbox_machines_headless.webp" srcset="https://sottomano.work/recordings/virtualbox_machines_headless.webp 2x" alt="Opening the VirtualBox machines group from the Virtualization menu and hovering a headless machine">

</div>

<div class="beside">
<div class="beside-text">

A VMware machine.

</div>

<img src="https://sottomano.work/recordings/vmware_machines_workstation.webp" srcset="https://sottomano.work/recordings/vmware_machines_workstation.webp 2x" alt="Opening the VMware machines group from the Virtualization menu and hovering a machine">

</div>

<div class="beside">
<div class="beside-text">

A running LXC container.

</div>

<img src="https://sottomano.work/recordings/lxc_containers_running.webp" srcset="https://sottomano.work/recordings/lxc_containers_running.webp 2x" alt="Opening the LXC containers group from the Virtualization menu and hovering a running container">

</div>

<div class="beside">
<div class="beside-text">

An Apptainer container started from an image.

</div>

<img src="https://sottomano.work/recordings/apptainer_containers_image.webp" srcset="https://sottomano.work/recordings/apptainer_containers_image.webp 2x" alt="Opening the Apptainer containers group from the Virtualization menu and hovering a container started from an image">

</div>

<div class="beside">
<div class="beside-text">

The Waydroid session.

</div>

<img src="https://sottomano.work/recordings/waydroid_container_session.webp" srcset="https://sottomano.work/recordings/waydroid_container_session.webp 2x" alt="Opening the Waydroid group from the Virtualization menu and hovering its session">

</div>

<div class="beside">
<div class="beside-text">

A running Flatpak app.

</div>

<img src="https://sottomano.work/recordings/flatpak_sandboxes_app.webp" srcset="https://sottomano.work/recordings/flatpak_sandboxes_app.webp 2x" alt="Opening the Flatpak apps group from the Virtualization menu and hovering a running app">

</div>

<div class="beside">
<div class="beside-text">

The CI runners slice against its memory cap.

</div>

<img src="https://sottomano.work/recordings/cgroup_memory_limits_ci_runners.webp" srcset="https://sottomano.work/recordings/cgroup_memory_limits_ci_runners.webp 2x" alt="Opening the memory limits group from the Virtualization menu and hovering the CI runners slice">

</div>
