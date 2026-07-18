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

## Current state (Phase 1 done; Phase 2 sysfs group done; Phase 3 group/menu system started)

Multi-surface daemon via `iced_layershell::build_pattern::daemon` + `#[to_layer_message(multi)]`.
`StartMode::Background` opens no surface up-front; `Bar` enumerates niri outputs and opens one
`NewLayerShell` per (output, edge) itself, bound by name via `OutputOption::OutputName`. Files:

- `src/main.rs` — `daemon()` builder with `StartMode::Background` + `--smoke`.
- `src/bar_program.rs` — Elm `Bar` state with shared data (clock/battery/niri) + a
  `BTreeMap<window::Id, Surface>` of open surfaces; `#[to_layer_message(multi)] Message`,
  `view(&self, id)` renders the surface's own output's workspaces. `close_events()` drops a
  surface's state on teardown. Layer-shell surface/output bookkeeping (`Surface`, `Position`,
  `layer_settings`) lives in `src/layer_surface.rs`; the async polling/subscription glue
  (`poll`, `niri_action`, `niri_subscription`, `now_hm`, `SourceKind`) lives in
  `src/message_subscriptions.rs`.
- `src/data_sources/niri.rs` — `niri-ipc` event-stream thread → tokio mpsc → iced Subscription
  (replaces the ~380-line derivation in `../odin/DataProvider/NiriService.qml`) +
  `list_outputs()` one-shot `Request::Outputs` enumeration.
- `src/user_config.rs` (+ `user_config/{color_palette,font_config,layout_config}.rs`) — reads
  odin's **own** `$XDG_CONFIG_HOME/odin/config.json` (shared for now; the dark/light symlink is
  followed) into `palette`/`font`/`layout`, with odin's exact gruvbox-dark defaults. Schema is
  `../odin/UserConfig/*.qml`. Later we split to our own file.
- `src/concrete_widgets.rs` (+ `concrete_widgets/{workspace_pager,window_title_list}.rs`) and
  `src/widget_groups.rs` (+ `widget_groups/{battery_group,system_group,workspaces_group,
  window_list_group,clock_group}.rs`) — odin's bar as data (Phase 3 start): `layout(position)`
  (in `widget_groups.rs`) returns per-bar `Zone`s
  (`shell.qml`'s exact top/bottom composition & order — top and bottom now differ), each holding
  ordered `Group`s. A `Group` is `Static` (odin's `WidgetGroup`: a row of widgets), `Menu` (odin's
  `MenuGroup`: a handle that expands content **in place** — no popup — under a `Hairline` affordance
  that's faint at rest / brighter on hover / solid when open), `Workspaces`, or `WindowList`.
  Source-backed leaf widgets render as a Font Awesome icon (**`iced_font_awesome`** `fa_icon_solid`,
  odin's `icon` names verbatim; self-registers the FA font) + a thin vertical `gauge` (odin's
  `PercentGauge`), tinted by `level`. Unavailable sources render nothing (empty groups vanish).
  The `gauge`/`hairline`/`edge_overlay`/`leaf`/`font_family`/`level_color` rendering primitives
  (odin's `ReusableComponent` equivalent) live in `src/visual_elements.rs`, not inside
  `concrete_widgets`/`widget_groups` — they're what widgets are drawn with, not a widget.
  Font: odin's `sans-serif`/`monospace` are resolved to the same concrete families Qt uses (IBM
  Plex Sans / Lilex Nerd Font Mono, from `fc-match`) so text matches odin's rendering; iced loads
  system fonts so `Font::with_name` finds them.
  - **Menu interaction**: single-open (odin's `GroupMenuState`) — `open_menu`/`hovered_menu` on
    `Bar`; a menu handle click toggles (`ToggleMenu`), hover drives the hairline (`HoverMenu`), and
    a click on the bar background collapses any open menu (`CloseMenus`, via a dismiss `mouse_area`
    stacked *under* the content so widget clicks are captured first). odin's `System` group is the
    flagship: cpu handle → temperature/memory/disk expand.
  - **Hairline indicators** (this pass): workspace pills and window-list entries now carry a
    `Hairline` along the inner screen edge (powerTint focused, criticalTint urgent) via the shared
    `edge_overlay` helper, instead of text-color tinting — matching odin's focus-is-an-underline
    convention. Labels stay neutral/dim, not colored, except the focused/urgent entry's own text
    which goes full white. Not yet ported: the *sliding* animation between slots (odin's
    `Behavior on x/width`) — the indicator currently snaps to the focused slot.
  - **App icons** (this pass): `src/data_sources/app_icon.rs` ports `WindowIcon._resolveAppId` — `.desktop` entry
    lookup (`freedesktop-desktop-entry`, scanned once at startup off the executor) → its `Icon=`
    field resolved through the icon theme (`freedesktop-icons`, respecting `default_theme_gtk`) →
    theme-name variants of the app_id itself (as-is / lowercased / last dotted segment) → generic
    `application-x-executable` fallback. Rendered via iced's `image` widget (needs the `image`
    Cargo feature). `DesktopEntries` is loaded once into `Bar` behind an `Arc` (cheap to clone
    through `Message`, which iced requires `Clone` on) and passed through `Context`.
  - **Click-to-focus**: both the workspace pager and window list now dispatch niri
    `Action::FocusWorkspace`/`FocusWindow` on click (`niri::focus_workspace`/`focus_window`,
    fire-and-forget via the shared `niri_action` helper — the resulting state change arrives back
    through the event stream, no local optimistic update).
  - **Still to port** (Phase 3 cont.): most groups whose data sources are unported (Weather,
    network, media, audio, tray, notifications, quicklaunch) are omitted, not stubbed; the sliding
    hairline animation; per-window tick row above each workspace slot (`WorkspaceWindowIndicators`);
    click-on-active-workspace → toggle niri overview; drag-to-reorder for both pagers.
- `src/data_sources.rs` (+ `data_sources/disk.rs`, `data_sources/sysfs_sensors.rs` +
  `data_sources/sysfs_sensors/{battery,power,cpu_usage,cpu_clock,memory,temperature}.rs`) —
  native data sources (Phase 2). Each
  is a rich domain type (real fields, status enums, `Option` for "no hardware") rather than one
  stringly JSON bag, sharing two traits: `BarWidget` (render: icon/label/level/value/tooltip) and
  `Source` (`poll()`). **No Nu subprocess** — all reads are direct `std::fs` on `/proc`/`/sys`
  (`disk` via `statvfs(2)`). `script.rs` (the old Nu-runner) is deleted.

**Styling (matches odin, consuming its config):** the layer surface is fully transparent; the
chrome fill is `surface` at `bgOpacity` (rounded only when floating) painted by the view container —
so the active light config's `bgOpacity: 0` yields a genuinely see-through bar. Text is white
(`Config.barText`), sized/spaced from the config. Each bar reserves its strip via
`exclusive_zone = barOuterHeight` (odin's behaviour), so niri tiles windows clear of it.

**Deferred to Phase 4 — the backdrop.** odin's white text is legible only because niri casts a
dark drop-shadow behind it (off-screen `odin-backdrop-{pos}` caster + a niri `layer-rule`). odin-rs
has none yet, so over light wallpaper the text is low-contrast. Reproducing it is Phase 4 item 5
(niri blur region / gradient backdrop) — an off-screen caster surface per bar + a niri layer-rule,
entangled with the run-below-odin dev arrangement. Left transparent (odin-faithful) until then.

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

### Phase 2 — port the 49 Nu scripts → native Rust data sources (IN PROGRESS)
**Hard rule (maintainer):** no reuse of the `.nu` scripts and no subprocesses — every source is
in-process Rust. `script.rs` is deleted; there is no Nu fallback.

**Done — sysfs/procfs group** (`data_sources/sysfs_sensors/`, `data_sources/disk.rs`): battery, power-consumption,
cpu, cpu-clock, memory, temperature (hwmon, not `sensors`), disk (`statvfs`; the btrfs block-group
detail is deferred to a future btrfs-ioctl pass). All seven wired into the bar and rendered with FA
icons + level tints. `backlight` is **not** in this group despite the table below — it's clightd /
wl-gammarelay D-Bus with write actions, so it moves to the D-Bus phase.

Remaining categories below are unported.

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
- **Selected/focused state → the widget's own style, not a separate indicator element.**
  Render "this item is selected" as a plain `iced::widget::button`'s own
  `.style(|theme, status| ...)` closure (background/border keyed on the item's
  `is_focused`/`is_urgent`, folded with iced's built-in `Hovered`/`Pressed`
  `button::Status`), not a second sized/positioned widget (a hairline, an overlay)
  laid on top of or beside the content. This is how real shipped iced panels do it
  (cosmic-panel's workspace/app-list applets: `button::custom(content).class(state_fn)`)
  — see `src/visual_elements.rs`'s `button_style` — and it sidesteps a real
  `Length::Fill`-vs-`Shrink`-compression subtlety a separate overlay/underline element
  must navigate correctly (documented in git history if you need the trace). When a
  group of entries only some of which are selected/urgent, always build every entry
  through the *same* code path (button with a style closure), varying only the
  captured color/state — never conditionally swap between "plain" and "decorated"
  element shapes across renders, which defeats iced's widget-tree diffing.
- **Known limitation — the bottom bar can stop repainting.** `WAYLAND_DEBUG=1` protocol
  tracing has shown the bottom bar's `wl_surface` stop receiving `wl_callback.done`
  frame-pacing events a few commits after startup, while the top bar's surface keeps
  receiving them normally — confirmed independent of widget/data-layer choices (persists
  with or without images in the window list, and across an unrelated widget-implementation
  rewrite). Likely a `layershellev`/`iced_layershell` bug in per-surface frame-callback
  bookkeeping when one client owns multiple layer-shell surfaces (`daemon()` +
  `NewLayerShell`, this app's Phase 1 architecture) — every cosmic-panel reference
  applet avoids the problem structurally by running **one layer-shell surface per
  process** instead. If this becomes blocking, that one-surface-per-process model is the
  next thing to try; it's a real architectural change, not a quick fix.

## Reference docs

- `README.md` — quick run/verify.
- `~/.claude/plans/would-it-be-possible-parsed-patterson.md` — the original feasibility plan.
- `../odin` — the QML + Nu source being ported (behavioral source of truth).
