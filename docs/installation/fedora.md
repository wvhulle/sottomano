# Fedora

The Fedora-specific parts of installing sottomano: the build headers for
[cargo](from-source.md) and the one command that sets up everything else the
widgets read. The bar itself builds [from source](from-source.md) or comes from
[Nix](nix.md#on-other-distributions), and starts with the session as
[the session page](../session.md) shows.

## Build headers

Before the first `cargo` build, which needs the headers sottomano links
against:

```bash
sudo dnf install gcc pkgconf-pkg-config clang clang-devel \
  wayland-devel libxkbcommon-devel mesa-libGL-devel vulkan-loader-devel \
  systemd-devel pipewire-devel
```

## System setup

`sottomano setup` writes the system files and runs the system commands the
widgets read: the modules and udev rules, the runtime tools the widgets call,
and the journal group. Review everything it does first:

```bash
sottomano setup fedora --print
```

then run it:

```bash
sudo sottomano setup fedora
```

It is idempotent: files whose contents already match are left alone. Steps can
be left out with `--skip` (`rapl`, `amd-uncore`, `journal`,
`runtime-tools`), repeatable or comma-separated.

Fedora ships `tuned-ppd`, which serves the power-profiles-daemon D-Bus API the
power group reads. Tailscale comes from
[its own repository](https://tailscale.com/download/linux), and VPNs show up
when NetworkManager runs them through a VPN plugin, such as
`NetworkManager-openconnect` or `NetworkManager-openvpn`. Fedora uses the
`wheel` group; what each system piece is for is on the
[system setup page](system-setup.md).