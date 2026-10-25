# Installation

The flake provides a home-manager module and, for the parts that need root, a
NixOS module. Other distributions are untested.

## Try it from a checkout

Inside the Nix devshell (`nix develop`):

```bash
cargo r
```

Run `sottomano --help` to list the options that describe its setup without
starting the shell.

## Home-manager

Import `homeModules.default` from the flake. It installs the package, writes
`~/.config/sottomano.toml` and makes niri include `~/.config/sottomano/niri.kdl`
(the layer rules sottomano writes for its own surfaces). Unless
`services.sottomano.systemdUnit` is disabled, it also adds a systemd user
service bound to `graphical-session.target`.

## NixOS

A home-manager module cannot start the system daemons several widgets read. On
NixOS, import `nixosModules.default` and set `programs.sottomano.enable = true`.
This also unlocks root-only CPU readings and the X3D preference switch. On
machines shared with untrusted users, turn
`programs.sottomano.cpuPowerReading.enable` off.

Next: [configuration](configuration.md).
