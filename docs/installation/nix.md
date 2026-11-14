# Nix

The flake provides a package, a home-manager module for the bar and a NixOS
module for the system pieces some widgets read. NixOS is the tested setup; the
`nix profile` route below works on any distribution.

## On NixOS

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
  }

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
itself, as [its page](../compositors/hyprland.md#window-styling-rules) shows.

## System module

`programs.sottomano.enable` turns on fwupd, UPower and power-profiles-daemon (at
`mkDefault`, so a host using tlp can turn the last one off) and loads
`drivetemp` for SATA drive temperatures. The other options:

```nix
{
  programs.sottomano = {
    enable = true;
    journalReaders = [ "alice" ];
    containerControllers = [ "alice" ];
    autoBrightness.enable = true;
    cpuPowerReading.enable = true;
    cacheCounters.enable = true;
    x3dPreferenceSwitching.enable = true;
  };
}
```

- `journalReaders` adds users to `systemd-journal` so the systemd and journal
  groups can read the system journal. Members of `wheel` already can.
- `containerControllers` adds users to the `sottomano-containers` group,
  whose members may start, stop and restart systemd-nspawn and NixOS
  containers, microVMs and the Waydroid container from their switches: a
  polkit rule allows exactly those verbs on `container@*.service`,
  `systemd-nspawn@*.service`, `microvm@*.service` and
  `waydroid-container.service`, and nothing else. Any app running as a member
  can use it, and group changes apply at the next login.
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

## Home-manager module

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

- `settings` takes the keys of the
  [full default](../configuration.md#full-default) as Nix attributes and is
  written verbatim to `~/.config/sottomano.toml`.
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
- `compositors` (default `[ "niri" "sway" "hyprland" ]`) lists the
  compositors compiled into the bar. Listing only yours gives a smaller build;
  the bar still finds the running one at startup, and names the missing entry
  if it runs under a compositor left out of the list.
- `systemdUnit` (default `true`) adds a user service bound to
  `graphical-session.target`. Disable it to start sottomano from the
  compositor's own startup commands instead.

The home-manager modules also work with
[standalone home-manager](https://github.com/nix-community/home-manager) on
other distributions.

## On other distributions

`nix profile` builds the same package as on NixOS:

```bash
sh <(curl -L https://nixos.org/nix/install) --daemon
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
nix profile install git+https://codeberg.org/wvhulle/sottomano
```

A Nix-built binary looks for GPU drivers under `/run/opengl-driver`, which only
NixOS provides. If it fails to open its surfaces, start it through
[nixGL](https://github.com/nix-community/nixGL) or build it
[from source](from-source.md) instead. The system pieces some widgets read need
a manual [system setup](system-setup.md).