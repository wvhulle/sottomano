# Virtual machines and containers

## Services: Virtual machines

Every running QEMU virtual machine on this computer, one row each, with its CPU and memory use: the ones you start yourself, NixOS tests running inside a nix build, and libvirt domains. A paused or shutting-down machine says so in its row. The card lists its cores, memory, disks, forwarded ports and, for NixOS, the system it boots.

Pressing a machine that has its own window focuses that window. Right-pressing powers it off the way the power button would, when the machine has a QMP socket this user can open, or when it is a libvirt domain.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/virtual_machines.webp" width="1162" alt="Opening the virtual machines group from the Services menu and hovering a machine started from a terminal">

## Services: nspawn containers

Every systemd-nspawn container on this computer, one row each, including NixOS containers: the running ones machined knows about and the stopped ones that only have a `container@` or `systemd-nspawn@` service. A running container shows its address in its row; the others say whether they are stopped, starting or stopping. The card lists its service, addresses, uptime, leader process and root directory.

Pressing the switch on a row starts or stops the container's service. Containers are system services, so this needs your user in the system module's `nspawnContainerControllers` (or another polkit rule that lets it manage these units); otherwise the switch falls back to the state it had.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/nspawn_containers.webp" width="958" alt="Opening the nspawn containers group from the Services menu and hovering a running NixOS container">

## Services: Podman containers

Every Podman engine a systemd service on this computer serves, one row each, named after its owner, with the number of containers running in it. An engine whose socket this user may not open says so in its row. The card lists the running containers with their image, CPU and memory use, and the engine's unit and socket.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/podman_engines.webp" width="1" alt="Opening the Podman containers group from the Services menu">
