# Vetrata

My personal desktop shell for the [niri](https://github.com/YaLTeR/niri)
compositor: two panels in one binary, built on [iced](https://iced.rs) and
[iced_layershell](https://crates.io/crates/iced_layershell), reading D-Bus with
`zbus` where a service exists and calling the CLI where one does not.

The shell explains itself: hover any reading for a card with what it shows,
and the card's top line says what a click does.

## Tour

### Hardware

CPU, memory and temperature, with live detail on hover.

<img src="recordings/compute.webp" width="554" alt="Expanding the compute group and hovering CPU, memory and temperature">

### Weather and location

Current conditions with the hourly forecast, and where the location fix came from.

<img src="recordings/weather.webp" width="404" alt="Hovering the temperature and the location in the weather group">

### AI usage

Subscription windows, their projected use at reset, and extra spend.

<img src="recordings/ai-usage.webp" width="366" alt="Hovering the Claude usage windows">

### Media

The playing track with its progress and where to find it.

<img src="recordings/media.webp" width="394" alt="Hovering the playing track">

### Idle and lock

Idle timeouts as sliders, each saying when it fires and how.

<img src="recordings/idle-lock.webp" width="363" alt="Expanding the idle timeouts and hovering the lock timer">

### Inline sliders

Volume and microphone level as sliders that open in place, with a live level meter.

<img src="recordings/audio-sliders.webp" width="390" alt="Opening the volume slider, dragging it and hovering the microphone">

### Wallpaper

The place in the current wallpaper and its Wikipedia summary.

<img src="recordings/wallpaper.webp" width="707" alt="Expanding the wallpaper group and hovering the article">

### Workspaces and windows

A preview of each workspace, every window, and the process tree behind the focused one.

<img src="recordings/bottom-bar.webp" width="618" alt="Hovering a workspace, a window and its process inspector">

### Workspace sidebar

Workspaces can also sit in a vertical sidebar, with the same previews.

<img src="recordings/workspace-sidebar.webp" width="419" alt="Hovering each workspace in the vertical sidebar">

## Run

Inside the Nix devshell (`nix develop`):

```bash
cargo r
```

`vetrata --help` lists the options that describe its setup instead of starting
the shell.

## Install

Import `homeModules.default` from the flake. It installs the package, writes
`~/.config/vetrata.toml`, makes niri include `~/.config/vetrata/niri.kdl`
(the layer rules vetrata writes for its own surfaces) and, unless
`services.vetrata.systemdUnit` is disabled, a systemd user service bound to
`graphical-session.target`.

Several widgets read system daemons a home-manager module cannot start. On
NixOS, import `nixosModules.default` and set `programs.vetrata.enable = true`.
It also unlocks root-only CPU readings and the X3D preference switch; turn
`programs.vetrata.cpuPowerReading.enable` off on machines shared with untrusted
users.

## Configure

`$XDG_CONFIG_HOME/vetrata.toml`, or `services.vetrata.settings`, which is
written to it verbatim. `vetrata --print-default-config` prints every key with
its default. Unknown keys are rejected. The bar applies every change to the
file in place; an invalid file raises a toast and keeps the previous
configuration.

Temperature sensors are detected automatically, so their keys don't appear in
the printed defaults. To pin a sensor or override a threshold:

```toml
[temperature.cpu]
sensor = { chip = "k10temp", label = "Tctl" }
critical_celsius = 90
```

## Secrets

`vetrata --list-secrets` lists the optional secret files, what each enables and
whether it is present. `services.vetrata.secretFiles.<name>` symlinks each from
wherever the secret actually lives, so nothing lands in the nix store.
