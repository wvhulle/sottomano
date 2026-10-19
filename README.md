<img src="logo.svg" width="96" alt="sottomano logo">

# Sottomano

*Sottomano* is Italian for "at hand": *avere tutto sottomano*, to have everything
within reach. It is my personal desktop shell for the
[niri](https://github.com/YaLTeR/niri) compositor. The aim is modest: keep every
reading one glance or hover away, so the panels inform without asking for
attention.

What it is:

- A top panel, a bottom panel, an optional workspace sidebar and notification
  toasts, all in one binary.
- Written in Rust on [iced](https://iced.rs), through a
  [fork](https://github.com/wvhulle/iced) that carries a small rendering patch.
- Drawn as Wayland layer surfaces with `iced_exwlshell` from waycrate's
  [exwlshelleventloop](https://github.com/waycrate/exwlshelleventloop), again
  through a [fork](https://github.com/wvhulle/exwlshelleventloop) with the
  patches the shell needs until they land upstream.
- Fed by [niri-ipc](https://github.com/YaLTeR/niri) for the compositor, by
  D-Bus through [zbus](https://dbus2.github.io/zbus/) where a service exists,
  by PipeWire for audio, and by a CLI where nothing else is available.

How it explains itself:

- Hover any reading for a card that says what it shows.
- The card's top line says what a click does.

The tour below is rendered from fixtures, so the numbers are made up, and it
covers only the groups that have a recording. The shell also has groups for the
clock, disks, GPU, network, power, the system tray, systemd units and a few
more, which look and behave the same way.

## Top panel

The panel I look at most. Each group sits folded to a few glyphs and expands
on hover or click; most controls open in place rather than in a window.

### Compute

CPU load, clock and temperature, with live detail on hover. Nothing here that
`top` would not tell you, but it is always in view.

#### CPU

Pressing the CPU reading unfolds the whole group.

<img src="recordings/compute_cpu.webp" width="720" alt="Expanding the compute group from the CPU reading">

#### Temperature

The hottest sensor, with the card saying which chip it is reading.

<img src="recordings/compute_temperature.webp" width="720" alt="Hovering temperature in the expanded compute group">

### Memory and board

Memory use with the processes holding the most, and the board and firmware
behind it.

#### Memory

Pressing the memory reading unfolds the group.

<img src="recordings/memory_board_memory.webp" width="720" alt="Expanding the memory and board group from the memory reading">

### Weather

Current conditions with the hourly forecast, and where the location fix came
from. It is a thin view on a public weather service, nothing more.

#### Conditions

Pressing the temperature opens the forecast for the coming hours.

<img src="recordings/weather_conditions.webp" width="720" alt="Pressing the temperature in the weather group">

#### Location

The card is honest about how rough the fix is and which source produced it.

<img src="recordings/weather_location.webp" width="720" alt="Hovering the location in the weather group">

### AI usage

Subscription windows, their projected use at reset, and extra spend. Useful if
you pay for an AI subscription and want to notice before it runs out.

#### Usage windows

Each window shows how much is used and where it is heading by the time it resets.

<img src="recordings/ai_usage_windows.webp" width="720" alt="Expanding the AI usage group and hovering the Claude usage windows">

### Media

The playing track with its progress and where to find it, read from any
player that speaks MPRIS.

#### Track

Hovering the track names the player and the album, with the progress underneath.

<img src="recordings/media_track.webp" width="720" alt="Hovering the playing track">

### Idle timeouts

Idle timeouts as sliders, each saying when it fires and how. Dragging a slider
changes the timeout; there is no separate settings page.

#### Screen off

The screen-off timer, with the card saying what happens when it fires.

<img src="recordings/idle_screen_off.webp" width="720" alt="Expanding the idle timeouts and hovering the screen-off timer">

#### Lock

The lock timer, which sits next to the screen-off timer so the two can be read together.

<img src="recordings/idle_lock.webp" width="720" alt="Expanding the idle timeouts and hovering the lock timer">

### Audio

Volume and microphone level as sliders that open in place, with a live level
meter. It talks to PipeWire directly, so it follows whatever device is active.

#### Volume

Clicking the speaker opens the slider where the glyph was; dragging it sets the level.

<img src="recordings/audio_volume.webp" width="720" alt="Opening the volume slider and dragging it">

#### Microphone

The microphone reading in the expanded group, with its meter moving while you speak.

<img src="recordings/audio_microphone.webp" width="720" alt="Expanding the audio group and hovering the microphone">

### Wallpaper

The place in the current wallpaper and its Wikipedia summary. A small
indulgence: it only works when the wallpaper carries a location.

#### Place

Expanding the group from the place name the lookup found.

<img src="recordings/wallpaper_place.webp" width="720" alt="Expanding the wallpaper group from the place lookup">

#### Article

The Wikipedia summary for that place, shown on hover.

<img src="recordings/wallpaper_article.webp" width="720" alt="Hovering the Wikipedia article in the wallpaper group">

## Bottom panel

Where the windows live. It is meant for finding things, not for decoration.

### Workspaces

A preview of each workspace, so you can see what is on it before switching.

#### Previews

Hovering a workspace shows its windows at a glance.

<img src="recordings/bottom_workspaces.webp" width="720" alt="Hovering a workspace in the bottom panel">

### Windows

Every window, and the process tree behind the focused one. The latter is
mostly there for finding out what a terminal is actually running.

#### Window

Hovering a window shows its title, application and workspace.

<img src="recordings/bottom_window.webp" width="720" alt="Hovering a window in the bottom panel">

#### Process inspector

The process tree behind a window, read from the window's PID.

<img src="recordings/bottom_process_inspector.webp" width="720" alt="Hovering the process inspector of a window">

## Workspace sidebar

An alternative to the bottom panel's workspace strip for wide screens. It is
the same widget, turned vertical.

### Workspaces

Workspaces can also sit in a vertical sidebar, with the same previews.

#### Previews

Hovering each workspace in turn.

<img src="recordings/sidebar_workspaces.webp" width="720" alt="Hovering each workspace in the vertical sidebar">

## Overlays

Surfaces that only appear when something happens, and leave when it is dealt
with.

### Notification toasts

Notifications appear as toasts, next to the window that sent them when it is
on screen. The shell is the notification daemon, so there is nothing else to
run.

#### Arrival

A toast arrives, holds while hovered and expires on its own.

<img src="recordings/toast_arrival.webp" width="720" alt="A toast arriving, holding while hovered and expiring">

#### Progress

A download that reports progress draws a bar instead of repeating itself.

<img src="recordings/toast_progress.webp" width="720" alt="A download toast with a rising progress bar">

#### Actions

Actions the sender offers appear as buttons in the toast.

<img src="recordings/toast_actions.webp" width="720" alt="Hovering the actions of an invitation toast and accepting it">

#### Next to its window

When the sending window is visible, the toast sits beside it instead of in a corner.

<img src="recordings/toast_window_anchored.webp" width="720" alt="A build toast placed beside the terminal that sent it">

## Run

For trying it out from a checkout. Inside the Nix devshell (`nix develop`):

```bash
cargo r
```

`sottomano --help` lists the options that describe its setup instead of starting
the shell.

## Install

The flake provides a home-manager module and, for the parts that need root, a
NixOS module. Other distributions are untested.

Import `homeModules.default` from the flake. It installs the package, writes
`~/.config/sottomano.toml`, makes niri include `~/.config/sottomano/niri.kdl`
(the layer rules sottomano writes for its own surfaces) and, unless
`services.sottomano.systemdUnit` is disabled, a systemd user service bound to
`graphical-session.target`.

Several widgets read system daemons a home-manager module cannot start. On
NixOS, import `nixosModules.default` and set `programs.sottomano.enable = true`.
It also unlocks root-only CPU readings and the X3D preference switch; turn
`programs.sottomano.cpuPowerReading.enable` off on machines shared with untrusted
users.

## Configure

One TOML file, and the shell itself prints the complete default so there is no
option reference to keep in sync here.

`$XDG_CONFIG_HOME/sottomano.toml`, or `services.sottomano.settings`, which is
written to it verbatim. `sottomano --print-default-config` prints every key with
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

A few widgets need an API key. Each is optional, and the shell works without
them.

`sottomano --list-secrets` lists the optional secret files, what each enables and
whether it is present. `services.sottomano.secretFiles.<name>` symlinks each from
wherever the secret actually lives, so nothing lands in the nix store.
