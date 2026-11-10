# Ubuntu

The Ubuntu-specific parts of installing sottomano: the build headers for
[cargo](from-source.md), the runtime tools the widgets call, and the manual
system setup that reproduces what the
[NixOS module](nix.md#system-module) does. The bar itself comes from
[Nix](nix.md#on-other-distributions) or
[cargo](from-source.md), and starts with the session as
[the session page](../session.md) shows.

## Build headers

```bash
sudo apt install build-essential pkg-config clang libclang-dev \
  libwayland-dev libxkbcommon-dev libgl-dev libvulkan-dev \
  libudev-dev libsystemd-dev libpipewire-0.3-dev
```

## Runtime tools and services

The widgets call these tools and D-Bus services when present and hide what they
cannot read:

```bash
sudo apt install iproute2 network-manager modemmanager bluez inxi \
  pipewire wireplumber wireguard-tools xdg-utils wl-clipboard \
  fwupd upower power-profiles-daemon
```

Tailscale comes from [its own repository](https://tailscale.com/download/linux).
VPNs show up when NetworkManager runs them through a VPN plugin, such as
`network-manager-openconnect` or `network-manager-openvpn`.

## System setup

The equivalent of `programs.sottomano.enable` and its default options:

```bash
echo drivetemp | sudo tee /etc/modules-load.d/sottomano.conf
sudo modprobe drivetemp

sudo tee /etc/udev/rules.d/90-sottomano.rules <<'EOF'
ACTION=="add", SUBSYSTEM=="powercap", KERNEL=="intel-rapl:*", RUN+="/bin/chmod 0444 /sys%p/energy_uj"
ACTION=="add|bind", SUBSYSTEM=="platform", DRIVER=="amd_x3d_vcache", RUN+="/bin/chgrp sudo /sys%p/amd_x3d_mode", RUN+="/bin/chmod 0664 /sys%p/amd_x3d_mode"
EOF
sudo udevadm control --reload && sudo udevadm trigger

sudo usermod -aG systemd-journal "$USER"
```

Leave out the RAPL rule on machines shared with untrusted users: it is the
PLATYPUS power side channel. On AMD, add `amd_uncore` to the modules file and
`kernel.perf_event_paranoid = 0` to `/etc/sysctl.d/90-sottomano.conf` for the
per-chiplet cache counters. Ubuntu uses the `sudo` group where NixOS uses
`wheel`. What each piece is for is on the
[system setup page](system-setup.md).