# Virtualization and CI runs

The Virtualization menu holds everything that runs a guest on this computer: virtual machines, VirtualBox and VMware machines, microVMs, Incus instances, nspawn, LXC, Podman, Docker and Apptainer containers, Waydroid, Flatpak sandboxes, and the memory caps that keep them from crowding out the desktop. Groups for kinds of guests this computer does not run stay hidden.

Each guest's row shows what pressing it does:

- A **switch** means Sottomano can turn the guest both on and off. Pressing it stops a running guest and starts a stopped one, so stopped guests are listed too.
- A **stop square** means Sottomano can stop the guest but not start it again. Pressing it stops the guest, and the row goes away.
- A **muted switch** means the guest runs but Sottomano has no way to control it. Pressing it does nothing.

A press that the guest's manager refuses is written to Sottomano's log, and the row keeps its state. CPU use counts like `top`: 100 % is one fully busy core.

## Virtualization: QEMU machines

Every running QEMU virtual machine on this computer, one row each, with its CPU and memory use: the ones you start yourself, NixOS tests running inside a nix build, and libvirt domains. Shut-off libvirt domains on `qemu:///system` are listed after them. A paused or shutting-down machine says so in its row. The card lists its cores, memory, disks, forwarded ports and, for NixOS, the system it boots.

A libvirt domain has a switch: pressing it shuts a running domain down or starts a shut-off one. Any other machine with a QMP socket this user can open has a stop square, which powers it off the way the power button would. A machine with neither gets a muted switch, and its card says power control is unavailable. Right-pressing a machine that has its own window focuses that window.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/virtual_machines.webp" width="1096" alt="Opening the QEMU machines group from the Virtualization menu and hovering a machine started from a terminal">

## Virtualization: VirtualBox machines

Every running VirtualBox machine on this computer, one row each, with its memory use: the ones run headless and the ones in their own window. The machines registered with `VBoxManage list vms` that are not running are listed after them as stopped. The card lists whether it has a window, its memory use, guest memory and cores, uptime, settings file and machine id; a stopped machine's card lists its id.

Pressing the switch on a running machine presses its power button through `VBoxManage controlvm`; on a stopped machine it starts it with `VBoxManage startvm`.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/virtualbox_machines_headless.webp" width="1054" alt="Opening the VirtualBox machines group from the Virtualization menu and hovering a headless machine">

## Virtualization: VMware machines

Every running VMware machine on this computer, one row each, with its memory use. The card lists its memory use, guest system, guest memory and cores, uptime and configuration file. Pressing a machine's stop square shuts its guest down with `vmrun stop`.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/vmware_machines_workstation.webp" width="952" alt="Opening the VMware machines group from the Virtualization menu and hovering a machine">

## Virtualization: microvm.nix guests

Every guest running under a lightweight hypervisor on this computer, one row each, with its memory use: Cloud Hypervisor, Firecracker, crosvm and kvmtool, as started by microvm.nix or by hand. Every installed `microvm@` service has a row, running or stopped, named after the service's instance; a running one's memory is that service's. Any other guest is named after its `--id` or `--name`, or its hypervisor and process. The card lists the hypervisor, guest memory and cores, uptime, service, kernel and API socket.

Pressing the switch on a microvm.nix guest starts or stops its service, which needs your user in the system module's `containerControllers`. A guest started by hand has a stop square when it is a Cloud Hypervisor or Firecracker guest with an API socket this user can open, which presses its power button, and a muted switch otherwise.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/microvm_guests_cloud_hypervisor.webp" width="897" alt="Opening the microvm.nix guests group from the Virtualization menu and hovering a Cloud Hypervisor guest">

## Virtualization: Incus instances

Every Incus or LXD instance on this computer, containers and virtual machines alike, across all projects, one row each. A running instance shows its address, or its memory use when it has none; a stopped one says so. The card lists its kind, status, daemon, project, memory use, addresses and image. Pressing the switch on a stopped instance starts it, and on a running one stops it. Reading the daemon needs your user in the `incus-admin` (or `lxd`) group; without it the daemon gets one row that says access is denied.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/incus_instances_container.webp" width="793" alt="Opening the Incus instances group from the Virtualization menu and hovering a container">

## Virtualization: nspawn containers

Every systemd-nspawn container on this computer, one row each, including NixOS containers: the running ones machined knows about and the stopped ones that only have a `container@` or `systemd-nspawn@` service. A running container shows its address in its row; the others say whether they are stopped, starting or stopping. The card lists its service, addresses, uptime, memory use, leader process and root directory.

Pressing the switch on a row starts or stops the container's service. Containers are system services, so this needs your user in the system module's `containerControllers` (or another polkit rule that lets it manage these units); otherwise the switch falls back to the state it had.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/nspawn_containers.webp" width="975" alt="Opening the nspawn containers group from the Virtualization menu and hovering a running NixOS container">

## Virtualization: LXC containers

Every plain LXC container on this computer, one row each: the running ones, found by their control group, and the stopped ones configured under `/var/lib/lxc`. A running container shows its memory use. The card lists its state, memory use, configured addresses and config file. Waydroid's container is left out here because it has its own group.

Pressing the switch on a running container stops it with `lxc-stop`, and on a stopped one starts it with `lxc-start`. Both run as your user, so a container only root may manage refuses the press.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/lxc_containers_running.webp" width="781" alt="Opening the LXC containers group from the Virtualization menu and hovering a running container">

## Virtualization: Podman containers

Every container in the Podman engines that systemd services on this computer serve, one row each: the running ones first, with their memory use, then the stopped ones, most recently stopped first. A list longer than twelve rows ends in a row that counts the rest. A container's memory is what its systemd scope uses minus the file cache it could drop, as `podman stats` counts it. The card lists its state, image, CPU and memory use, uptime, name and engine owner. A Woodpecker step is shown by its step name.

Pressing the switch on a running container stops it, giving it ten seconds to exit, and on a stopped one starts it. An engine whose socket this user may not open gets one row with a warning, and its card says why and lists the engine's unit and socket.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/podman_engines.webp" width="648" alt="Opening the Podman containers group from the Virtualization menu">

## Virtualization: Docker containers

Every container in the Docker engines on this computer, one row each: the system engine at `/run/docker.sock` and a rootless engine in your runtime directory. An engine that is not installed or not running adds nothing. Running containers come first, with their memory use, then the stopped ones, newest first. A list longer than twelve rows ends in a row that counts the rest. A container's memory is what its systemd scope uses minus the file cache it could drop, as `docker stats` counts it. The card lists its state, image, CPU and memory use, uptime, name and engine. A Woodpecker step is shown by its step name.

Pressing the switch on a running container stops it, giving it ten seconds to exit, and on a stopped one starts it. An engine whose socket this user may not open gets one row with a warning.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/docker_engines_rootful.webp" width="917" alt="Opening the Docker containers group from the Virtualization menu and hovering a container in the system engine">

## Virtualization: Apptainer containers

Every running Apptainer or Singularity container on this computer, one row each, named after its image, with its memory use. The card lists the image file, the command it runs, memory use, uptime, owner and runtime. Pressing a container's stop square sends its runtime a termination signal.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/apptainer_containers_image.webp" width="939" alt="Opening the Apptainer containers group from the Virtualization menu and hovering a container started from an image">

## Virtualization: Waydroid

Waydroid as two rows: its container, and the Android session inside it, which is listed while the container runs. The container row shows its memory use while it runs; its card lists its state, its `waydroid-container.service` unit and its memory use. The session row shows the session's state; its card lists the state, the user and the screen density.

Pressing the container's switch starts or stops `waydroid-container.service`, which needs your user in the system module's `containerControllers`. Before Sottomano has read the service's state, the container row shows a muted switch. Pressing the session's switch starts the session with `waydroid session start`, or stops it with `waydroid session stop`.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/waydroid_container_session.webp" width="705" alt="Opening the Waydroid group from the Virtualization menu and hovering its session">

## Virtualization: Flatpak apps

Every Flatpak app you are running, one row per sandbox, with its memory use. The card lists the app, memory use, uptime, runtime, branch, instance number and sandbox process. Pressing an app's stop square quits it with `flatpak kill`.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/flatpak_sandboxes_app.webp" width="1012" alt="Opening the Flatpak apps group from the Virtualization menu and hovering a running app">

## Virtualization: Memory limits

Every systemd slice, service or container on this computer that has a memory cap, one row each, with its memory use against the cap, plus the machine slice where containers and libvirt machines run, capped or not. A row warns when its use passes the mark where the kernel starts throttling it, or when its processes spent more than a tenth of the last ten seconds waiting for memory. It turns red when the kernel killed one of its processes since Sottomano started.

The card lists the throttle mark, the hard cap, swap in use against its own cap, the time spent waiting for memory, how often the cgroup was throttled or hit the cap, how many processes were killed, and the cgroup path. Pressing a row opens `systemd-cgtop` in a terminal, sorted by memory, for that cgroup.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/cgroup_memory_limits_ci_runners.webp" width="977" alt="Opening the memory limits group from the Virtualization menu and hovering the CI runners slice">

## Services: CI runs

The runs that the Woodpecker and Forgejo runners on this computer are executing, one row each, with its repository and run and how long it has been running. The card shows the step that is running, its image, CPU and memory use, the time until it times out and a link to its log.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/ci_runner_tasks.webp" width="751" alt="Opening the CI runs group from the Services menu and hovering a Woodpecker run">
