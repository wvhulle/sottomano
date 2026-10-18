# Contributing

Conventions and workflow for working on sottomano.

## Workflow

Work inside the Nix devshell (`nix develop`). The repository is a cargo
workspace: `sottomano-config/` holds the TOML config types and their loading,
`sottomano-sources/` holds the data sources and sinks the shell reads and writes,
`sottomano-state/` holds the runtime state the shell persists and restores,
`sottomano-widgets/` holds the widgets, widget groups, hover tooltips and the
`Message` they produce, `sottomano-shell/` composes them into the running shell
(binary `sottomano`) and `sottomano-widget-showcase/` plays widget fixtures for the
README recordings, the `.ice` scenarios and the windowed tester. `cargo r` for a
debug run, `cargo clippy --workspace --all-targets` before committing — the
workspace warns on clippy pedantic plus `absolute_paths` — and `cargo fmt`, which
needs a nightly toolchain for the options in `.rustfmt.toml`. Version control is jj, not the `git` CLI.

### Debugging with comet

`cargo r --features devtools` attaches iced's devtools to the running shell, and
`--features time-travel` adds rewinding. The metrics show in comet, iced's
debugger window. The bar is a layer surface that never takes keyboard focus, so
devtools' F12 shortcut does not reach it. Start comet yourself:

```bash
cargo install --locked --git https://github.com/iced-rs/comet.git \
  --rev 3f75f3240edc1719df584810337bc7df010327d8
iced_comet &
cargo r --features devtools
```

The revision is the one the iced fork's devtools pins (`COMPATIBLE_REVISION` in
`iced_devtools/src/comet.rs`); bump it when the fork moves. The shell's beacon
keeps retrying its connection, so comet and the shell can start in either order.
Comet then shows each message with its update, view, layout and draw timings,
plus task and subscription counts; under `time-travel` it can rewind the shell
to an earlier message.

Woodpecker CI (`.woodpecker/`) builds the flake `checks` — tests, clippy and
rustfmt — on pull requests and `main`, and checks pull request commits against
Conventional Commits (`nix run --inputs-from . nixpkgs#convco -- check main..HEAD`). After checks
pass on `main`, the `readme-recordings` pipeline re-renders the tour and opens
a pull request when the recordings changed.

## Conventions

- No comments, doc-comments or docstrings. Intent lives in names and call flow;
  code that cannot be made self-evident that way is restructured, not annotated.
- Strict config/state separation: no field appears in both the TOML config
  (`sottomano-config/src/`) and runtime state (`sottomano-state/src/`).
- No `macro_rules!` deduplication. Use traits, methods and marker types.
- No global config statics. Config is threaded through as instance data.
- Widgets over free helper functions: style functions, builder widgets and
  methods on the owning type (`sottomano-widgets/src/widgets/`,
  `sottomano-widgets/src/widget_groups/`).
- Widget state lives in its widget and routes through its group's `Action` —
  no `App`-level carve-outs, no shadow copies of a cell's state.
- Names describe structure, not appearance.
- No user instructions in `README.md` beyond running, installing,
  configuring and the recorded tour with one-line captions. The UX should be
  intuitive; where help or explanation is needed, keep it next to the widget
  in question through descriptive labels, tooltip cards and visual indicators
  rather than prose in the docs.
- No config keys, defaults or TOML snippets in `README.md`. Users get a
  complete default config from `sottomano --print-default-config` and start
  from there, so every option must show up in that output with its default.

## README recordings

The tour recordings in `recordings/` are rendered offscreen by the
`sottomano-widget-showcase` crate (`nix build .#readme-recordings` renders them all
the way CI does). Each entry of `DEMOS` in `demo_script.rs` shows one
widget, is named `<group>_<widget>` and sits in `README.md` under the heading of its
panel and group. The name is also the demo's iced `Preset`, which boots its fixture
for the recorder, the `.ice` scenarios and the windowed tester alike. It names a
fixture and a script of steps: `Hover` / `Press` /
`Drag` on `HoverTarget`s for panel widgets, and `HoverText` / `PressText` /
`TypeReply` on visible text for toasts, which a `ShowcasedGroup::Toasts` fixture
serves when the demo starts. The recorder finds each target on screen, plays the
steps through `iced_test`'s emulator and writes `recordings/<name>.webp`, a lossless animated WebP that spans
the full canvas width with its height cropped to what the demo shows.

To add a demo, write `fixtures/<group>.rs` (the group fed its readings through
its own messages), give the group a variant in `showcased_group.rs` if it has
none yet, and add a `Demo` entry. A target that is not on screen fails the run
and lists the targets that are.

```bash
cargo run -p sottomano-widget-showcase
cargo run -p sottomano-widget-showcase -- weather_location
cargo run -p sottomano-widget-showcase -- --window
```

`--window` opens the presets in a real window with the iced tester attached,
which switches between presets and records interactions as `.ice` scenarios.
Scenarios saved in `sottomano-widget-showcase/tests/ice/` run with `cargo test`
(and so in CI); `expect` sees plain text widgets, not rich text such as toast
bodies.

The full run also checks that `README.md` embeds every recording as
`<img src="recordings/<name>.webp" width="720">`, the logical canvas width, so all
demos show at the same scale; it prints the expected tag when one is off.

## Upstream docs

- [iced book](https://book.iced.rs/) and [API docs](https://docs.iced.rs/)
- [iced_layershell](https://github.com/waycrate/exwlshelleventloop) (and the
  fork carrying the active patch)
- [niri IPC](https://github.com/YaLTeR/niri) for the compositor protocol
- [zbus](https://dbus2.github.io/zbus/) for D-Bus data sources

`README.md` covers running, installing and configuring the shell.
