# odin-rs

Native-Rust port of the Odin status bar for the [niri](https://github.com/YaLTeR/niri)
compositor. One compiled binary built on
[iced](https://iced.rs) + [iced_layershell](https://crates.io/crates/iced_layershell):
no QML, no Nushell subprocesses.

## Status

See `TODO.md` for the full phased plan. Verified end-to-end on a single-output
niri session: top + bottom bars render with the correct exclusive zones, live
niri workspaces, and the sysfs/procfs data sources (battery, cpu, memory,
temperature, disk, power).

## Verified stack

- `iced` 0.14, `iced_layershell` 0.19.1 (vendored as a git dep on a fork —
  see `TODO.md`), `niri-ipc` 26.4 (matches niri 26.04).
- Runtime: a niri session.
- Dev environment: Nix flake with a nightly Rust toolchain
  (rust-src, rustfmt, clippy, rust-analyzer) and the system libraries
  needed at build/link/dlopen time (wayland, libxkbcommon, libGL,
  vulkan-loader).

## Build

```sh
nix develop -c cargo build
```

Or build a runnable bundle in the Nix store:

```sh
nix build .#default
./result/bin/odin-rs
```

## Launch

Requires a running niri session for the GUI (layer surface + IPC socket).

```sh
nix develop -c cargo run             # launch the bar
nix develop -c cargo run -- --smoke  # headless: print parsed data-layer output
```

`--smoke` runs the data-layer code paths without opening any wayland
surface, useful when only a data source changed.

## Configuration

The bar reads its config from `$XDG_CONFIG_HOME/odin.toml` — its own file,
not shared with Odin. Missing keys keep the gruvbox-dark defaults. Colors
are any CSS Color Level 4 string the `linebender/color` crate accepts
(hex, `rgb()`, `hsl()`, named, `oklab()`, …).

```toml
[palette]
bar_background = "#504945b3"   # bar panel fill (alpha honored)
inactive       = "#bdae93"     # non-focused window on an inactive workspace
urgent         = "#fb4934"     # urgent state + critical severity
warning        = "#fabd2f"     # warning severity
focus          = "#8ec07c"     # focus indicator + focused window
dim            = "#FFFFFF80"   # fixed dim foreground (superscript prefix)

[layout]
corner_radius = 8
vertical_padding = 3
horizontal_group_padding = 6
horizontal_widget_spacing = 5
horizontal_group_spacing = 9
floating = true
blur = true
shadow = false
float_offset = 0.0
focus_underline_thickness = 3.4
window_segment_thickness = 2.0
```

Text uses `iced::Font::DEFAULT` / `iced::Font::MONOSPACE` from the
user's login-session font configuration — no font overrides in the
odin-rs config.

## Limitations

- **Backdrop** — Odin casts a dark drop-shadow behind its white text via
  an off-screen layer + a niri `layer-rule`. odin-rs renders against a
  transparent layer, so over light wallpaper the text is low-contrast.
  Reproducing the backdrop trick is on the roadmap (see `TODO.md`).
- **Hot reload** — none for a compiled binary; runtime config requires a
  restart until the config-file layer lands.
- **Hotplug** — output removal is handled; output addition needs a
  manual re-enumeration trigger (deferred, see `TODO.md`).

For service-level limitations (popups, tray, notifications, MPRIS,
blurred backdrop) see `TODO.md`.
