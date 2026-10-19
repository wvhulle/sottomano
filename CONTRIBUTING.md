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

Debugging the running shell with iced's devtools and comet is described in
`sottomano-shell/CONTRIBUTING.md`.

Woodpecker CI (`.woodpecker/`) builds the flake `checks` — tests, clippy and
rustfmt — on pull requests and `main`, and checks pull request commits against
Conventional Commits (`nix run --inputs-from . nixpkgs#convco -- check main..HEAD`). After checks
pass on `main`, the `readme-recordings` pipeline re-renders the tour and opens
a pull request when the recordings changed.

## Conventions

- No comments, doc-comments or docstrings. Intent lives in names and call flow;
  code that cannot be made self-evident that way is restructured, not annotated.
- `sottomano-sources/src/` is grouped by domain. Each directory covers what one
  widget domain reads and carries two or more keywords
  (`cpu_memory_compute/`, `login_session_presence/`); each file is named after
  its subject and the mechanism behind it (`journal_oom_kills`,
  `gammarelay_color_temperature`, `upower_battery_state`).
  `system_protocol_clients/` holds the D-Bus, HTTP, shell, inotify, sysfs and
  journal access that names no daemon and implements no `ReadingSource`.
  Consumers spell out the new module path rather than aliasing it back to a
  short name.
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

The tour in `README.md` is rendered by `sottomano-widget-showcase`. How the
recordings are scripted, added and checked is described in
`sottomano-widget-showcase/CONTRIBUTING.md`.

## Upstream docs

- [iced book](https://book.iced.rs/) and [API docs](https://docs.iced.rs/)
- [exwlshelleventloop](https://github.com/waycrate/exwlshelleventloop), the
  source of `iced_exwlshell` (and the fork carrying the active patches)
- [niri IPC](https://github.com/YaLTeR/niri) for the compositor protocol
- [zbus](https://dbus2.github.io/zbus/) for D-Bus data sources

`README.md` covers running, installing and configuring the shell.
