# Installation

Sottomano runs on [niri](compositors/niri.md), [sway](compositors/sway.md) and
[Hyprland](compositors/hyprland.md), and finds the running one from
`NIRI_SOCKET`, `HYPRLAND_INSTANCE_SIGNATURE` or `SWAYSOCK`. Each compositor's
page covers installing it, starting the session and including the window
styling rules sottomano writes, and lists what that compositor does not offer.
The flake provides a package, a home-manager module for the bar and a NixOS module
for the system pieces some widgets read. NixOS is the tested setup. The Ubuntu
and Fedora instructions reproduce what the NixOS module does by hand and are
untested.

## Try it from a checkout

On any distribution with Nix and flakes enabled:

```bash
git clone https://codeberg.org/wvhulle/sottomano
cd sottomano
nix develop
cargo r
```

Or run the packaged binary without cloning:

```bash
nix run git+https://codeberg.org/wvhulle/sottomano
```

`sottomano --help` lists the subcommands that describe the setup without
starting the shell, such as `default-config` and `secrets`.

## NixOS

Add the flake as an input and import both modules. The NixOS module enables the
system services and kernel modules. The home-manager module installs the bar,
writes its config and starts it with the graphical session.

```nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    sottomano = {
      url = "git+https://codeberg.org/wvhulle/sottomano";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, home-manager, sottomano, ... }:
    {
      nixosConfigurations.laptop = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          ./configuration.nix
          sottomano.nixosModules.default
          home-manager.nixosModules.home-manager
          {
            programs.sottomano.enable = true;
            home-manager.users.alice = {
              imports = [ sottomano.homeModules.default ];
              services.sottomano.enable = true;
            };
          }
        ];
      };
    };
}
```

`sottomano.homeModules.default` also imports niri-hm's module. When
home-manager runs niri (`services.niri.enable`) or sway
(`wayland.windowManager.sway.enable`), the module includes the window styling
rules sottomano writes for that compositor. Hyprland's Lua config loads them
itself, as [its page](compositors/hyprland.md#window-styling-rules) shows.

### System module

`programs.sottomano.enable` turns on fwupd, UPower and power-profiles-daemon (at
`mkDefault`, so a host using tlp can turn the last one off) and loads
`drivetemp` for SATA drive temperatures. The other options:

```nix
{
  programs.sottomano = {
    enable = true;
    journalReaders = [ "alice" ];
    nspawnContainerControllers = [ "alice" ];
    autoBrightness.enable = true;
    cpuPowerReading.enable = true;
    cacheCounters.enable = true;
    x3dPreferenceSwitching.enable = true;
  };
}
```

- `journalReaders` adds users to `systemd-journal` so the systemd and journal
  groups can read the system journal. Members of `wheel` already can.
- `nspawnContainerControllers` lets users start, stop and restart
  systemd-nspawn and NixOS containers from the nspawn containers group: a
  polkit rule allows exactly those verbs on `container@*.service` and
  `systemd-nspawn@*.service`, and nothing else.
- `autoBrightness` runs clight for webcam-based brightness, which drives the
  monitor group's auto toggle and backlight slider.
- `cpuPowerReading` (on by default) makes the RAPL energy counters
  world-readable for the CPU power reading. That is the PLATYPUS power side
  channel, so turn it off on machines shared with untrusted users.
- `cacheCounters` (on by default with AMD microcode) loads `amd_uncore` and sets
  `kernel.perf_event_paranoid = 0` for per-chiplet L3 hit rates.
- `x3dPreferenceSwitching` (on by default) lets `wheel` switch the X3D cache or
  frequency preference from the bar. It matches no device on CPUs without the
  `amd_x3d_vcache` driver.

### Home-manager module

```nix
{ config, ... }:
{
  services.sottomano = {
    enable = true;
    umbra.enable = false;
    windowAgents.kitty.enable = true;
    secretFiles = {
      caldav-password = config.age.secrets.caldav-password.path;
      gh-auth-token = config.age.secrets.gh-auth-token.path;
    };
  };
}
```

- `settings` takes the keys of the [full default](configuration.md#full-default)
  as Nix attributes and is written verbatim to `~/.config/sottomano.toml`.
- `secretFiles.<name>` symlinks each secret into `~/.config/sottomano/` from
  wherever it lives (agenix, sops-nix), so it never enters the Nix store.
- `umbra.enable` (default `true`) registers dark and light palettes with
  [umbra](https://codeberg.org/wvhulle/umbra) and requires its home-manager
  module. Set it to `false` for a static palette.
- `windowAgents.kitty.enable` turns on kitty's socket-only remote control so the
  window list can tell kitty OS windows apart.
- `virtualMachines.qmpSocket.enable` (default `true`) sets `QEMU_OPTS` so every
  NixOS `run-*-vm` script you start opens a QMP socket in its own temporary
  directory, which the virtual machines group uses to show a paused VM and power
  it off. An existing `QEMU_OPTS` wins.
- `systemdUnit` (default `true`) adds a user service bound to
  `graphical-session.target`. Disable it to start sottomano from the
  compositor's own startup commands instead.

## Ubuntu

Install the compositor first, as its page describes.

### Build

The quickest route is Nix, which builds the same package as on NixOS:

```bash
sh <(curl -L https://nixos.org/nix/install) --daemon
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
nix profile install git+https://codeberg.org/wvhulle/sottomano
```

A Nix-built binary looks for GPU drivers under `/run/opengl-driver`, which only
NixOS provides. If it fails to open its surfaces, start it through
[nixGL](https://github.com/nix-community/nixGL) or build it with cargo. The
home-manager module also works with standalone home-manager on Ubuntu and
Fedora.

To build with cargo instead, install the headers and a nightly toolchain
(rustup picks it up from `rust-toolchain.toml`):

```bash
sudo apt install build-essential pkg-config clang libclang-dev \
  libwayland-dev libxkbcommon-dev libgl-dev libvulkan-dev \
  libudev-dev libsystemd-dev libpipewire-0.3-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The icon crates embed SVGs from three icon sets at build time, so check them
out at the revisions the flake pins and point the build at them:

```bash
mkdir -p ~/src && cd ~/src
git clone https://github.com/phosphor-icons/core phosphor-core
git -C phosphor-core checkout 2b75f3ad12b420c9504ef05df8d2564a28f8500e
git clone --depth 1 --branch 16.35.0 https://github.com/simple-icons/simple-icons
git clone --depth 1 --branch 2.0.12 https://github.com/erikflowers/weather-icons

git clone https://codeberg.org/wvhulle/sottomano && cd sottomano
export PHOSPHOR_SVG=~/src/phosphor-core/assets
export SIMPLE_ICONS_SVG=~/src/simple-icons/icons
export WEATHER_ICONS_SVG=~/src/weather-icons/svg
cargo install --locked --path sottomano-shell
```

### Runtime tools and services

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

### System setup

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

Leave out the RAPL rule on machines shared with untrusted users. On AMD, add
`amd_uncore` to the modules file and `kernel.perf_event_paranoid = 0` to
`/etc/sysctl.d/90-sottomano.conf` for the per-chiplet cache counters. Ubuntu
uses the `sudo` group where NixOS uses `wheel`.

### Start with the session

See [Start with the session](#start-with-the-session).

## Fedora

Install the compositor first, as its page describes.

### Build

With Nix, as on Ubuntu and with the same GPU driver caveat:

```bash
sh <(curl -L https://nixos.org/nix/install) --daemon
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
nix profile install git+https://codeberg.org/wvhulle/sottomano
```

With cargo, install these headers, then follow the rustup, icon set and
`cargo install` steps from [Ubuntu](#build):

```bash
sudo dnf install gcc pkgconf-pkg-config clang clang-devel \
  wayland-devel libxkbcommon-devel mesa-libGL-devel vulkan-loader-devel \
  systemd-devel pipewire-devel
```

### Runtime tools and services

```bash
sudo dnf install iproute NetworkManager ModemManager bluez inxi \
  pipewire wireplumber wireguard-tools xdg-utils wl-clipboard \
  fwupd upower tuned-ppd
```

Fedora ships `tuned-ppd`, which serves the power-profiles-daemon D-Bus API the
power group reads. Tailscale comes from
[its own repository](https://tailscale.com/download/linux).
VPNs show up when NetworkManager runs them through a VPN plugin, such as
`NetworkManager-openconnect` or `NetworkManager-openvpn`.

### System setup

The same modules-load, udev and journal steps as on
[Ubuntu](#system-setup), with `wheel` in place of `sudo` in the X3D rule.

## Start with the session

Outside home-manager, write the user service the module would generate to
`~/.config/systemd/user/sottomano.service`:

```ini
[Unit]
Description=sottomano bar
PartOf=graphical-session.target
After=graphical-session.target

[Service]
ExecStart=%h/.cargo/bin/sottomano
Restart=on-failure
RestartSec=2
Slice=session.slice
CPUWeight=500

[Install]
WantedBy=graphical-session.target
```

Use `%h/.nix-profile/bin/sottomano` for a Nix profile install, then enable it:

```bash
systemctl --user enable --now sottomano.service
```

`session.slice` and the raised weight keep the bar responsive when apps load the
machine. The service starts with `graphical-session.target`. niri starts that
target itself; sway and Hyprland need a line in their config, as their pages
show.

Next: [configuration](configuration.md).
