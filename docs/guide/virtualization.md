# Virtualization and CI runs

The Virtualization menu holds everything that runs a guest on this computer: virtual machines, VirtualBox and VMware machines, microVMs, Incus instances, nspawn, LXC, Podman, Docker and Apptainer containers, Waydroid, Flatpak sandboxes, and the memory caps that keep them from crowding out the desktop. Groups for kinds of guests this computer does not run stay hidden.

## Virtualization: Virtual machines

Every running QEMU virtual machine on this computer, one row each, with its CPU and memory use: the ones you start yourself, NixOS tests running inside a nix build, and libvirt domains. A paused or shutting-down machine says so in its row. The card lists its cores, memory, disks, forwarded ports and, for NixOS, the system it boots.

Pressing a machine that has its own window focuses that window. Right-pressing powers it off the way the power button would, when the machine has a QMP socket this user can open, or when it is a libvirt domain.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/virtual_machines.webp" width="1059" alt="Opening the virtual machines group from the Virtualization menu and hovering a machine started from a terminal">

## Virtualization: VirtualBox machines

Every running VirtualBox machine on this computer, one row each, with its memory use: the ones run headless and the ones in their own window. The card lists whether it has a window, its memory use, guest memory and cores, uptime, settings file and machine id.

Right-pressing a machine presses its power button through `VBoxManage controlvm`.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/virtualbox_machines_headless.webp" width="1054" alt="Opening the VirtualBox machines group from the Virtualization menu and hovering a headless machine">

## Virtualization: VMware machines

Every running VMware machine on this computer, one row each, with its memory use. The card lists its memory use, guest system, guest memory and cores, uptime and configuration file.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/vmware_machines_workstation.webp" width="952" alt="Opening the VMware machines group from the Virtualization menu and hovering a machine">

## Virtualization: MicroVMs

Every guest running under a lightweight hypervisor on this computer, one row each, with its memory use: Cloud Hypervisor, Firecracker, crosvm and kvmtool, as started by microvm.nix or by hand. A microvm.nix guest is named after its `microvm@` service and its memory is that service's; any other guest is named after its `--id` or `--name`, or its hypervisor and process. The card lists the hypervisor, guest memory and cores, uptime, service, kernel and API socket.

Right-pressing a Cloud Hypervisor or Firecracker guest with an API socket this user can open presses its power button.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/microvm_guests_cloud_hypervisor.webp" width="897" alt="Opening the MicroVMs group from the Virtualization menu and hovering a Cloud Hypervisor guest">

## Virtualization: Incus instances

Every Incus or LXD instance on this computer, containers and virtual machines alike, across all projects, one row each. A running instance shows its address, or its memory use when it has none; a stopped one says so. The card lists its kind, status, daemon, project, memory use, addresses and image. Pressing a stopped instance starts it and right-pressing a running one stops it. Reading the daemon needs your user in the `incus-admin` (or `lxd`) group; without it the daemon gets one row that says access is denied.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/incus_instances_container.webp" width="793" alt="Opening the Incus instances group from the Virtualization menu and hovering a container">

## Virtualization: nspawn containers

Every systemd-nspawn container on this computer, one row each, including NixOS containers: the running ones machined knows about and the stopped ones that only have a `container@` or `systemd-nspawn@` service. A running container shows its address in its row; the others say whether they are stopped, starting or stopping. The card lists its service, addresses, uptime, memory use, leader process and root directory.

Pressing the switch on a row starts or stops the container's service. Containers are system services, so this needs your user in the system module's `nspawnContainerControllers` (or another polkit rule that lets it manage these units); otherwise the switch falls back to the state it had.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/nspawn_containers.webp" width="1015" alt="Opening the nspawn containers group from the Virtualization menu and hovering a running NixOS container">

## Virtualization: LXC containers

Every plain LXC container on this computer, one row each: the running ones, found by their control group, and the stopped ones configured under `/var/lib/lxc`. A running container shows its memory use. The card lists its state, memory use, configured addresses and config file. Waydroid's container is left out here because it has its own group.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/lxc_containers_running.webp" width="781" alt="Opening the LXC containers group from the Virtualization menu and hovering a running container">

## Virtualization: Podman containers

Every Podman engine a systemd service on this computer serves, one row each, named after its owner, with the number of containers running in it. An engine whose socket this user may not open shows a warning in its row, and its card says why. The card lists the running containers with their image, CPU and memory use, and the engine's unit and socket.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/podman_engines.webp" width="684" alt="Opening the Podman containers group from the Virtualization menu">

## Virtualization: Docker containers

The Docker engines on this computer, one row each: the system engine at `/run/docker.sock` and a rootless engine in your runtime directory. An engine that is not installed or not running has no row. Each row shows how many containers run in it, and its card lists them with their image, CPU and memory use. A Woodpecker step is shown by its step name.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/docker_engines_rootful.webp" width="917" alt="Opening the Docker containers group from the Virtualization menu and hovering the system engine">

## Virtualization: Apptainer containers

Every running Apptainer or Singularity container on this computer, one row each, named after its image, with its memory use. The card lists the image file, the command it runs, memory use, uptime, owner and runtime.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/apptainer_containers_image.webp" width="939" alt="Opening the Apptainer containers group from the Virtualization menu and hovering a container started from an image">

## Virtualization: Waydroid

The Android container Waydroid runs, as one row while its container or session is up, with the session's state. The card lists whether the container runs, its memory use, the session's state and user, and the screen density. Right-pressing it asks Waydroid to stop the container.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/waydroid_container_session.webp" width="705" alt="Opening the Waydroid group from the Virtualization menu and hovering its session">

## Virtualization: Flatpak apps

Every Flatpak app you are running, one row per sandbox, with its memory use. The card lists the app, memory use, uptime, runtime, branch, instance number and sandbox process.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/flatpak_sandboxes_app.webp" width="1012" alt="Opening the Flatpak apps group from the Virtualization menu and hovering a running app">

## Virtualization: Memory limits

Every systemd slice, service or container on this computer that has a memory cap, one row each, with its memory use against the cap, plus the machine slice where containers and libvirt machines run, capped or not. A row warns when its use passes the mark where the kernel starts throttling it, or when its processes spent more than a tenth of the last ten seconds waiting for memory. It turns red when the kernel killed one of its processes since Sottomano started.

The card lists the throttle mark, the hard cap, swap in use against its own cap, the time spent waiting for memory, how often the cgroup was throttled or hit the cap, how many processes were killed, and the cgroup path. Pressing a row opens `systemd-cgtop` in a terminal, sorted by memory, for that cgroup.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/cgroup_memory_limits_ci_runners.webp" width="1018" alt="Opening the memory limits group from the Virtualization menu and hovering the CI runners slice">

## Services: CI runs

The runs that the Woodpecker and Forgejo runners on this computer are executing, one row each, with its repository and run and how long it has been running. The card shows the step that is running, its image, CPU and memory use, the time until it times out and a link to its log.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/ci_runner_tasks.webp" width="794" alt="Opening the CI runs group from the Services menu and hovering a Woodpecker run">
