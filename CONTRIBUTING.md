# CONTRIBUTING — odin-rs

Guidance for an AI agent working in this repo. Read this first; it overrides defaults.

## What this repo is

**odin-rs is a ground-up rewrite of [Odin](../odin) in Rust.** Odin is a Quickshell/QML
desktop shell for the [niri](https://github.com/YaLTeR/niri) compositor: ~8,500 lines of
QML + ~4,600 lines of Nushell scripts. The end goal here is a **single pure-Rust binary**
built on [`iced`](https://iced.rs) + [`iced_layershell`](https://crates.io/crates/iced_layershell):

- **No QML** — every widget/bar is an iced view.
- **No Nushell** — every one of the 49 `scripts/*.nu` data sources becomes native Rust
  (D-Bus via `zbus`, HTTP via `reqwest`, sysfs/proc via direct fs, niri via `niri-ipc`).

`../odin` is the **behavioral source of truth**. When porting a widget or data source, read
its QML/Nu counterpart and match its behavior; verify parity visually (see below).

Motivation (from the maintainer): dislikes QML, wants one compiled binary with lower
overhead, and wants to fix accumulated bugs during the rewrite.

## Verified stack (all version-aligned, on this machine)

- `iced` 0.14, `iced_layershell` 0.19.1, `niri-ipc` 26.4.0 (matches niri 26.04).
- Build/link deps come from `shell.nix` (wayland, libxkbcommon, libGL, vulkan-loader).
- Toolchain: cargo/rustc nightly, present in the dev shell.

## THE feedback loop — how to reliably *see* your change

There is a live niri session with the **original odin bar running**. odin-rs is designed to
run *alongside* it and render **flush below odin's top bar** (it sets `exclusive_zone: 0`, so
niri drops it below odin's reserved top zone automatically). This lets you screenshot both
bars stacked and visually diff odin-rs against the reference. **Use this on every UI change —
do not trust "it compiles".**

```sh
# 1. build
nix-shell --run 'cargo build'

# 2. run alongside the live odin bar (background; keeps rendering)
WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/1000 \
  nix-shell --run './target/debug/odin-rs' &

# 3. screenshot. grim uses LOGICAL coordinates. Get output geometry first:
niri msg --json outputs        # DP-1 is currently 2258x1270 @ scale 1.7
grim -g "0,0 900x70" top.png   # top strip: odin bar + odin-rs below it

# 4. enlarge a region to read widgets, then view the PNG
magick top.png -resize 300% top-big.png
```

Then look at `top-big.png`: odin-rs shows an orange `odin-rs ▸` marker so it's unmistakable.
Compare its clock / battery / workspaces against odin's own bar in the same image.

- **Headless data-layer check** (no display): `nix-shell --run './target/debug/odin-rs --smoke'`
  runs the script + niri data paths and prints parsed output — use when you only changed a
  data source.
- Kill a running instance with `pkill -f target/debug/odin-rs` (⚠ don't put that exact
  string in a larger shell command — pkill will match its own wrapper).

## Current state (Phase 1 — multi-output daemon, DONE)

Multi-surface daemon via `iced_layershell::build_pattern::daemon` + `#[to_layer_message(multi)]`.
`StartMode::Background` opens no surface up-front; `Bar` enumerates niri outputs and opens one
`NewLayerShell` per (output, edge) itself, bound by name via `OutputOption::OutputName`. Files:

- `src/main.rs` — `daemon()` builder with `StartMode::Background` + `--smoke`.
- `src/app.rs` — Elm `Bar` state with shared data (clock/battery/niri) + a `BTreeMap<window::Id,
  Surface>` of open surfaces; `#[to_layer_message(multi)] Message`, `view(&self, id)` renders the
  surface's own output's workspaces. `close_events()` drops a surface's state on teardown.
- `src/niri.rs` — `niri-ipc` event-stream thread → tokio mpsc → iced Subscription (replaces the
  ~380-line derivation in `../odin/DataProvider/NiriService.qml`) + `list_outputs()` one-shot
  `Request::Outputs` enumeration.
- `src/script.rs` — **temporary** Nu-subprocess runner + JSON parse. Phase 2 deletes this as
  scripts are ported to native Rust.

Validated visually (single-output machine, DP-1): top bar lands flush below odin's QML top bar,
bottom bar above odin's bottom bar, each showing DP-1's workspaces with `[odin]` active — plus the
Phase 0 checks (anchored full-width bar + exclusive zone, subprocess→JSON→battery, live niri
workspaces).

**Hotplug — open question settled:** layershellev auto-creates a surface on a newly-connected
display **only** in `StartMode::AllScreens`, and even then with an internal `window::Id` we never
registered (so `view(id)` would miss our `surfaces` map — it renders empty, doesn't panic). In the
named-output `Background` mode we use, the new-display event is silently swallowed
(`DispatchMessageInner::NewDisplay` → the public conversion is `unimplemented!()`, never reached).
Output *removal* IS surfaced — layershellev closes the surface, arriving as `window::close_events`.
So: **removal is handled; addition needs a manual re-enumeration trigger** (re-run `list_outputs` +
`open_bars` for new names). Deferred to a later event-driven pass rather than a poll timer, per the
maintainer's distrust of always-on timers.

## Conversion plan (phased — keep each phase shippable & visually verified)

### Phase 1 — multi-output daemon ✅ DONE
Switched `application()` → `daemon()`, `#[to_layer_message(multi)]`, state keyed by `window::Id`
in a `BTreeMap`. Enumerate outputs (`niri_ipc::Request::Outputs`) and open one `NewLayerShell` per
(output, position: top & bottom, mirroring `../odin/shell.qml` `Variants`), bound by name via
`NewLayerShellSettings.output_option = OutputOption::OutputName`. Hotplug open question settled —
see "Current state" above (removal handled via `close_events`; addition needs a manual
re-enumeration trigger, deferred).

### Phase 2 — port the 49 Nu scripts → native Rust data sources
Each `../odin/scripts/*.nu` emits one JSON object matching the contract in
`../odin/ReusableComponent/ScriptDataSource.qml` (`icon`, `label`, `badge`, `level`, `tooltip`,
`available`, and a tagged-union `value` facet: `slider|gauge|chart|none`). That JSON **is the
spec**. Port each to a Rust module producing the same typed data as an async task/subscription,
then delete its `script::run` call. Group by source type:

| Category | Scripts | Rust approach |
|---|---|---|
| sysfs / procfs | battery, backlight, cpu, cpu-clock, memory, disk, temperature, power-consumption | direct `std::fs` / `sysinfo` / `procfs` |
| NetworkManager (D-Bus) | network, ethernet, wifi | `zbus` → `org.freedesktop.NetworkManager` |
| ModemManager (D-Bus) | mobile-broadband | `zbus` → `org.freedesktop.ModemManager1` |
| bluez (D-Bus) | bluetooth, radio | `zbus` → `org.bluez` |
| PipeWire/WirePlumber | wireplumber-sink, -source, -device-type, -watch | `pipewire` crate or WirePlumber D-Bus |
| power-profiles-daemon | power-profile | `zbus` → `net.hadess.PowerProfiles` |
| wl-gammarelay-rs | color-temperature | `zbus` (its D-Bus iface) |
| systemd / journald | systemd-failed, kernel-errors | `zbus` systemd manager / journal reader |
| niri | workspaces (native already), tray-activate, idle-mode, idle-timeout | `niri-ipc` |
| HTTP | weather-temp/-rain/-*-forecast, pollen, claude-usage-*, copilot-usage-*, openrouter-usage, nvidia-usage | `reqwest` (nvidia via NVML if local) |
| app / CLI | cliphist, firefox-favicon, spotify-like, quicklaunch, tailscale, vpn, stopwatch, umbra | clipboard/`rusqlite` (Firefox places), desktop-entry crate, tailscale LocalAPI, internal state |
| lifecycle (drop/replace) | paths, odin-reload, wait-network | fold into Rust consts / startup logic |

Keep `../odin/scripts` as the reference while porting; a ported source should produce
byte-equivalent widget state.

### Phase 3 — port QML widgets → iced components
Map `../odin`'s layers onto iced: `UserConfig` (6) → a Rust config/theme module;
`ReusableComponent` (27) → generic iced widgets (gauges, sliders, pagers, popups);
`ConcreteWidget` (16) + `WidgetGroup` (18) → per-widget view fns composed into the bars in
`shell.qml` order. Verify each against the live odin bar via the feedback loop.

### Phase 4 — the hard services (highest risk; do a spike before committing)
1. **Popups/menus** — `BarPopup`/`HoverPopup`/`MenuGroup`/inline sliders. iced_layershell's
   author calls popup support "only a toy"; this is the top risk. Spike ONE popup early.
2. **System tray** — StatusNotifierItem host + DBusMenu over `zbus` (iced has none).
3. **Notification server** — own `org.freedesktop.Notifications` over `zbus` + toast surfaces.
4. **MPRIS** — `mpris`/`zbus`.
5. **niri blur region** — likely no iced equivalent; the drop-shadow gradient backdrop trick
   (bottom-layer surface + niri layer-rules) may be reproducible compositor-side.

### Phase 5 — config, packaging, cutover
Port `ConfigLoader`/`UserConfig` to a config-file layer (no QML hot-reload equivalent). Add a
Nix build (crane + rust-overlay per the ../odin flake conventions; `makeWrapper --prefix PATH`
for any residual runtime deps). Replace odin in the home-manager module once at parity.

## Conventions

- **Verify visually.** Every widget/UI change → run alongside odin + screenshot + compare.
- **Parity first.** Match `../odin` behavior before improving it; note intentional deviations.
- Version control is **jj** (jujutsu), not `git` — see the maintainer's global rules.
- Small, shippable commits; keep the bar building & runnable at every step.
- The maintainer distrusts always-on/auto-heal timers — prefer signal/event-driven updates,
  infrequent polls, and visible failure over silent retry loops.

## Reference docs

- `README.md` — quick run/verify.
- `~/.claude/plans/would-it-be-possible-parsed-patterson.md` — the original feasibility plan.
- `../odin` — the QML + Nu source being ported (behavioral source of truth).
