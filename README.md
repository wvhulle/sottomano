# odin-spike

Feasibility spike for porting [Odin](../odin) (a Quickshell/QML bar for niri) to
**iced + iced_layershell**. Throwaway crate — validates the load-bearing mechanics
before committing to a full rewrite. See the full plan at
`~/.claude/plans/would-it-be-possible-parsed-patterson.md`.

## What it validates

| Target | What | Where |
|--------|------|-------|
| **A** | Anchored top bar + fixed exclusive zone on niri, no keyboard focus | `main.rs` `LayerShellSettings` |
| **B** | Subprocess → JSON → widget (reuses Odin's `battery.nu`) | `script.rs`, `Message::PollBattery` |
| **C** | Live niri workspaces via `niri-ipc` event stream (replaces `NiriService.qml`) | `niri.rs`, `Message::Niri` |
| **D** | Multi-monitor (one bar per output) | **phase 2 — not yet wired**, see below |

## Verified stack

- `iced` 0.14.0, `iced_layershell` 0.19.1, `niri-ipc` 26.4.0 (matches niri 26.04) — all
  version-aligned on this machine.
- The ~4,600 lines of Nu scripts in `../odin/scripts/` are reused verbatim.

## Run

Requires a running **niri** session for the GUI (layer surface + IPC socket).

```sh
nix develop -c cargo run             # launch the bar
nix develop -c cargo run -- --smoke  # headless: test the data layer, no display
```

`--smoke` exercises `script.rs` (parses `battery.nu`) and `niri.rs` (prints the first
~10 workspace events) so B and C logic can be checked without watching pixels.

## Manual verification (A–D), in a niri session

1. **A** — maximize a terminal; its top edge should sit at y = 32 px (not 0), and niri
   keyboard shortcuts / terminal input must work on the first keypress (bar never steals
   focus).
2. **B** — battery % and icon update; `chmod -x ../odin/scripts/battery.nu` (or break it)
   and confirm the clock keeps ticking and the last value is retained.
3. **C** — switch workspaces in niri; the `[active]`-bracketed pill should follow within a
   frame. `ps` should show **no** `niri msg` subprocess (it's the event stream, not polling).
4. **D** — phase 2 (below).

## Phase 2 — multi-output (D)

The spike runs a single `application()` bound to `StartMode::Active`. For one bar per
monitor, switch to the **daemon** builder (verified in iced_layershell's `multi_window`
example):

- `iced_layershell::build_pattern::daemon(Bar::new, "odin-spike", Bar::update, Bar::view)`
- `#[to_layer_message(multi)]` on `Message` (note the `(multi)`), state keyed by
  `window::Id` in a `BTreeMap`.
- At boot, enumerate outputs (via `niri_ipc::Request::Outputs`) and, per output, return
  `Message::NewLayerShell { settings: NewLayerShellSettings { anchor, exclusive_zone,
  keyboard_interactivity: None, output_option: <that output> }, id: window::Id::unique() }`.
- Route `view(id)` / per-surface state by `window::Id`; each bar renders
  `niri::workspaces_for_output(state, its_output_name)`.
- **Open question the phase-2 build settles:** whether `OutputOption` has a by-name/by-output
  variant (single-app `StartMode::TargetScreen("HDMI-1")` is confirmed), and whether output
  **hotplug** is observable so bars can be added/removed reactively.

## Known limitations / next risks (not in this spike)

- **Popups/menus** — iced_layershell's author calls popup support "only a toy". Odin's
  `BarPopup`/`HoverPopup`/`MenuGroup`/inline sliders are the top full-port risk; do a
  one-popup mini-spike before committing.
- Tray (StatusNotifierItem host), notification **server**, MPRIS — three D-Bus services
  Quickshell provides that iced does not; reimplement over `zbus`.
- niri blur region + drop-shadow gradient backdrop — blur likely has no iced equivalent.
- Hot reload — none for a compiled binary; runtime config needs a config-file layer.
