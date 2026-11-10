# Fedora

The Fedora-specific parts of installing sottomano: the build headers for
[cargo](from-source.md), the runtime tools the widgets call, and the manual
system setup that reproduces what the
[NixOS module](nix.md#system-module) does. The bar itself comes from
[Nix](nix.md#on-other-distributions) or
[cargo](from-source.md), and starts with the session as
[the session page](../session.md) shows.

## Build headers

```bash
sudo dnf install gcc pkgconf-pkg-config clang clang-devel \
  wayland-devel libxkbcommon-devel mesa-libGL-devel vulkan-loader-devel \
  systemd-devel pipewire-devel
```

## Runtime tools and services

The widgets call these tools and D-Bus services when present and hide what they
cannot read:

```bash
sudo dnf install iproute NetworkManager ModemManager bluez inxi \
  pipewire wireplumber wireguard-tools xdg-utils wl-clipboard \
  fwupd upower tuned-ppd
```

Fedora ships `tuned-ppd`, which serves the power-profiles-daemon D-Bus API the
power group reads. Tailscale comes from
[its own repository](https://tailscale.com/download/linux). VPNs show up when
NetworkManager runs them through a VPN plugin, such as
`NetworkManager-openconnect` or `NetworkManager-openvpn`.

## System setup

The same modules-load, udev and journal steps as on
[Ubuntu](ubuntu.md#system-setup), with `wheel` in place of `sudo` in the X3D
rule. What each piece is for is on the
[system setup page](system-setup.md).