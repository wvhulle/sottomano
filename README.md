<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/main/logo.svg" width="96" alt="sottomano logo">

# Sottomano

*Sottomano* is Italian for "at hand": *avere tutto sottomano*, to have everything
within reach. It is my desktop shell: a top panel, a bottom panel, an optional
workspace sidebar and notification toasts, in one binary.

## Why

After twenty years of GNOME and KDE Plasma I wanted a desktop that never asks
for attention but answers when I look, with the widgets I kept missing there.
Sottomano is written in Rust, so readings arrive as streams from compositor IPC,
D-Bus, PipeWire and native libraries instead of polled commands, and each one
explains itself in a hover card whose top line says what a click does.

Previous work:

- [niri-waybar-windowlist](https://github.com/wvhulle/niri-waybar-windowlist):
  a hard fork of a Waybar window list module, where styling meant fighting GTK
  CSS.
- [Odin](https://codeberg.org/wvhulle/odin): two Qt Quick panels on Quickshell,
  with a subprocess spawned and polled for every reading.

## Tour

### Weather: Conditions

Hovering the info icon of the temperature opens the forecast for the coming hours.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/weather_conditions.webp" width="861" alt="Hovering the temperature in the weather group">

### Tailscale: Exit node

The tailnet: how each peer is reached, when the key expires, and which exit
node carries the traffic, next to the switch that offers this machine as one.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/tailnet_exit_node.webp" width="883" alt="Opening the Tailscale group from the Network menu and hovering an exit node">

### Dictation: Speech model

The speech model the dictation daemon loaded.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/voice_dictation_model.webp" width="1163" alt="Opening the dictation group from the Media menu and hovering the speech model">

### Toast: Next to its window

When the sending window is visible, the toast sits in its top corner.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/toast_window_anchored.webp" width="689" alt="A build toast placed in the corner of the terminal that sent it">

The other groups, each in its menu next to pinned in the bar, are in the
[guide](https://sottomano.work/guide/). The recordings use fixtures, so the numbers are
made up.

## Built on

- [iced](https://iced.rs), through a [fork](https://github.com/wvhulle/iced)
  that carries a small rendering patch.
- Wayland layer surfaces from `iced_exwlshell` in waycrate's
  [exwlshelleventloop](https://github.com/waycrate/exwlshelleventloop), through
  a [fork](https://github.com/wvhulle/exwlshelleventloop) with the patches the
  shell needs until they land upstream.
- [niri-ipc](https://github.com/YaLTeR/niri) and
  [swayipc-types](https://github.com/jaycefayne/swayipc-rs) for compositor IPC,
  [zbus](https://z-galaxy.github.io/zbus/) for D-Bus services, PipeWire for audio,
  and a CLI only where nothing else is available.

## Get started

- Try it from a checkout: `cargo r` inside `nix develop`.
- Install it with the home-manager and NixOS modules:
  [installation](https://sottomano.work/installation/).
- Configure it with one TOML file whose complete default the shell prints:
  [configuration](https://sottomano.work/configuration.html).
- Work on it: [development](https://sottomano.work/development.html) and
  [style](https://sottomano.work/style.html).
