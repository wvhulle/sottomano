<img src="logo.svg" width="96" alt="sottomano logo">

# Sottomano

*Sottomano* is Italian for "at hand": *avere tutto sottomano*, to have everything
within reach. It is my desktop shell for [niri](https://github.com/YaLTeR/niri):
a top panel, a bottom panel, an optional workspace sidebar and notification
toasts, in one binary.

## Why

After twenty years of GNOME and KDE Plasma I wanted a niri desktop that never
asks for attention but answers when I look, with the widgets I kept missing
there. Sottomano is written in Rust, so readings arrive as streams from niri
IPC, D-Bus, PipeWire and native libraries instead of polled commands, and each
one explains itself in a hover card whose top line says what a click does.

Previous work:

- [niri-waybar-windowlist](https://github.com/wvhulle/niri-waybar-windowlist):
  a hard fork of a Waybar window list module, where styling meant fighting GTK
  CSS.
- [Odin](https://codeberg.org/wvhulle/odin): two Qt Quick panels on Quickshell,
  with a subprocess spawned and polled for every reading.

## Tour

### Weather: Conditions

Hovering the temperature opens the forecast for the coming hours.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/weather_conditions.webp" width="872" alt="Hovering the temperature in the weather group">

### VPN: Tailscale

The tailnet: how each peer is reached, the exit node and when the key expires,
next to the NetworkManager VPNs and WireGuard client and server tunnels.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/vpn_tailscale.webp" width="1031" alt="Opening the VPN group from the Network menu and hovering Tailscale">

### Dictation: Speech model

The speech model the dictation daemon loaded and the device it runs on.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/voice_dictation_model.webp" width="1174" alt="Opening the dictation group from the Media menu and hovering the speech model and compute device">

### Toast: Next to its window

When the sending window is visible, the toast sits in its top corner.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/toast_window_anchored.webp" width="688" alt="A build toast placed in the corner of the terminal that sent it">

The other groups, each in its menu next to pinned in the bar, are in the
[guide](docs/guide/README.md). The recordings use fixtures, so the numbers are
made up.

## Built on

- [iced](https://iced.rs), through a [fork](https://github.com/wvhulle/iced)
  that carries a small rendering patch.
- Wayland layer surfaces from `iced_exwlshell` in waycrate's
  [exwlshelleventloop](https://github.com/waycrate/exwlshelleventloop), through
  a [fork](https://github.com/wvhulle/exwlshelleventloop) with the patches the
  shell needs until they land upstream.
- [niri-ipc](https://github.com/YaLTeR/niri) for the compositor,
  [zbus](https://dbus2.github.io/zbus/) for D-Bus services, PipeWire for audio,
  and a CLI only where nothing else is available.

## Get started

- Try it from a checkout: `cargo r` inside `nix develop`.
- Install it with the home-manager and NixOS modules:
  [docs/installation.md](docs/installation.md).
- Configure it with one TOML file whose complete default the shell prints:
  [docs/configuration.md](docs/configuration.md).
- Work on it: [docs/development.md](docs/development.md) and
  [CONTRIBUTING.md](CONTRIBUTING.md).
