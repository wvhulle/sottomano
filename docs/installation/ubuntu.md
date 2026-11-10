# Ubuntu

The Ubuntu-specific parts of installing sottomano: the build headers for
[cargo](from-source.md) and the one command that sets up everything else the
widgets read. The bar itself builds [from source](from-source.md) or comes from
[Nix](nix.md#on-other-distributions), and starts with the session as
[the session page](../session.md) shows.

## Build headers

Before the first `cargo` build, which needs the headers sottomano links
against:

```bash
sudo apt install build-essential pkg-config clang libclang-dev \
  libwayland-dev libxkbcommon-dev libgl-dev libvulkan-dev \
  libudev-dev libsystemd-dev libpipewire-0.3-dev
```

## System setup

`sottomano setup` writes the system files and runs the system commands the
widgets read: the modules and udev rules, the runtime tools the widgets call,
and the journal group. Review everything it does first:

```bash
sottomano setup ubuntu --print
```

then run it:

```bash
sudo sottomano setup ubuntu
```

It is idempotent: files whose contents already match are left alone. Steps can
be left out with `--skip` (`rapl`, `amd-uncore`, `journal`,
`runtime-tools`), repeatable or comma-separated. Tailscale comes from
[its own repository](https://tailscale.com/download/linux), and VPNs show up
when NetworkManager runs them through a VPN plugin, such as
`network-manager-openconnect` or `network-manager-openvpn`.

Ubuntu uses the `sudo` group where other distributions use `wheel`; what each
system piece is for is on the [system setup page](system-setup.md).