# Virtualization and CI runs

The Virtualization menu holds everything that runs a guest on this computer: virtual machines, nspawn and Podman containers, and the memory caps that keep them from crowding out the desktop. Groups for kinds of guests this computer does not run stay hidden.

## Virtualization: Virtual machines

Every running QEMU virtual machine on this computer, one row each, with its CPU and memory use: the ones you start yourself, NixOS tests running inside a nix build, and libvirt domains. A paused or shutting-down machine says so in its row. The card lists its cores, memory, disks, forwarded ports and, for NixOS, the system it boots.

Pressing a machine that has its own window focuses that window. Right-pressing powers it off the way the power button would, when the machine has a QMP socket this user can open, or when it is a libvirt domain.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/virtual_machines.webp" width="1059" alt="Opening the virtual machines group from the Virtualization menu and hovering a machine started from a terminal">

## Virtualization: nspawn containers

Every systemd-nspawn container on this computer, one row each, including NixOS containers: the running ones machined knows about and the stopped ones that only have a `container@` or `systemd-nspawn@` service. A running container shows its address in its row; the others say whether they are stopped, starting or stopping. The card lists its service, addresses, uptime, memory use, leader process and root directory.

Pressing the switch on a row starts or stops the container's service. Containers are system services, so this needs your user in the system module's `nspawnContainerControllers` (or another polkit rule that lets it manage these units); otherwise the switch falls back to the state it had.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/nspawn_containers.webp" width="1015" alt="Opening the nspawn containers group from the Virtualization menu and hovering a running NixOS container">

## Virtualization: Podman containers

Every Podman engine a systemd service on this computer serves, one row each, named after its owner, with the number of containers running in it. An engine whose socket this user may not open shows a warning in its row, and its card says why. The card lists the running containers with their image, CPU and memory use, and the engine's unit and socket.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/podman_engines.webp" width="684" alt="Opening the Podman containers group from the Virtualization menu">

## Virtualization: Memory limits

Every systemd slice, service or container on this computer that has a memory cap, one row each, with its memory use against the cap, plus the machine slice where containers and libvirt machines run, capped or not. A row warns when its use passes the mark where the kernel starts throttling it, or when its processes spent more than a tenth of the last ten seconds waiting for memory. It turns red when the kernel killed one of its processes since Sottomano started.

The card lists the throttle mark, the hard cap, swap in use against its own cap, the time spent waiting for memory, how often the cgroup was throttled or hit the cap, how many processes were killed, and the cgroup path. Pressing a row opens `systemd-cgtop` in a terminal, sorted by memory, for that cgroup.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/cgroup_memory_limits_ci_runners.webp" width="1018" alt="Opening the memory limits group from the Virtualization menu and hovering the CI runners slice">

## Services: CI runs

The runs that the Woodpecker and Forgejo runners on this computer are executing, one row each, with its repository and run and how long it has been running. The card shows the step that is running, its image, CPU and memory use, the time until it times out and a link to its log.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/ci_runner_tasks.webp" width="794" alt="Opening the CI runs group from the Services menu and hovering a Woodpecker run">
