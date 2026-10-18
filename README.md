# Vetrata

My personal desktop shell for the [niri](https://github.com/YaLTeR/niri)
compositor: two panels in one binary, built on [iced](https://iced.rs) and
[iced_layershell](https://crates.io/crates/iced_layershell), reading D-Bus with
`zbus` where a service exists and calling the CLI where one does not.

The shell explains itself: hover any reading for a card with what it shows,
and the card's top line says what a click does.

## Top panel

### Compute

CPU, memory and temperature, with live detail on hover.

#### CPU

<img src="recordings/compute-cpu.webp" width="720" alt="Expanding the compute group from the CPU reading">

#### Memory

<img src="recordings/compute-memory.webp" width="720" alt="Hovering memory in the expanded compute group">

#### Temperature

<img src="recordings/compute-temperature.webp" width="720" alt="Hovering temperature in the expanded compute group">

### Weather

Current conditions with the hourly forecast, and where the location fix came from.

#### Conditions

<img src="recordings/weather-conditions.webp" width="720" alt="Pressing the temperature in the weather group">

#### Location

<img src="recordings/weather-location.webp" width="720" alt="Hovering the location in the weather group">

### AI usage

Subscription windows, their projected use at reset, and extra spend.

#### Usage windows

<img src="recordings/ai-usage-windows.webp" width="720" alt="Expanding the AI usage group and hovering the Claude usage windows">

### Media

The playing track with its progress and where to find it.

#### Track

<img src="recordings/media-track.webp" width="720" alt="Hovering the playing track">

### Idle timeouts

Idle timeouts as sliders, each saying when it fires and how.

#### Screen off

<img src="recordings/idle-screen-off.webp" width="720" alt="Expanding the idle timeouts and hovering the screen-off timer">

#### Lock

<img src="recordings/idle-lock.webp" width="720" alt="Expanding the idle timeouts and hovering the lock timer">

### Audio

Volume and microphone level as sliders that open in place, with a live level meter.

#### Volume

<img src="recordings/audio-volume.webp" width="720" alt="Opening the volume slider and dragging it">

#### Microphone

<img src="recordings/audio-microphone.webp" width="720" alt="Expanding the audio group and hovering the microphone">

### Wallpaper

The place in the current wallpaper and its Wikipedia summary.

#### Place

<img src="recordings/wallpaper-place.webp" width="720" alt="Expanding the wallpaper group from the place lookup">

#### Article

<img src="recordings/wallpaper-article.webp" width="720" alt="Hovering the Wikipedia article in the wallpaper group">

## Bottom panel

### Workspaces

A preview of each workspace.

#### Previews

<img src="recordings/bottom-workspaces.webp" width="720" alt="Hovering a workspace in the bottom panel">

### Windows

Every window, and the process tree behind the focused one.

#### Window

<img src="recordings/bottom-window.webp" width="720" alt="Hovering a window in the bottom panel">

#### Process inspector

<img src="recordings/bottom-process-inspector.webp" width="720" alt="Hovering the process inspector of a window">

## Workspace sidebar

### Workspaces

Workspaces can also sit in a vertical sidebar, with the same previews.

#### Previews

<img src="recordings/sidebar-workspaces.webp" width="720" alt="Hovering each workspace in the vertical sidebar">

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
