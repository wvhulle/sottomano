# Contributing

Conventions and workflow for working on vetrata.

## Workflow

Work inside the Nix devshell (`nix develop`). `cargo r` for a debug run,
`cargo clippy` before committing — the crate warns on clippy pedantic plus
`absolute_paths` — and `cargo fmt`, which needs a nightly toolchain for the
options in `rustfmt.toml`. Version control is jj, not the `git` CLI.

## Conventions

- No comments, doc-comments or docstrings. Intent lives in names and call flow;
  code that cannot be made self-evident that way is restructured, not annotated.
- Strict config/state separation: no field appears in both the TOML config
  (`src/config/`) and runtime state (`src/state/`).
- No `macro_rules!` deduplication. Use traits, methods and marker types.
- No global config statics. Config is threaded through as instance data.
- Widgets over free helper functions: style functions, builder widgets and
  methods on the owning type (`src/widgets/`, `src/widget_groups/`).
- Widget state lives in its widget and routes through its group's `Action` —
  no `App`-level carve-outs, no shadow copies of a cell's state.
- Names describe structure, not appearance.
- No user instructions in `README.md` beyond running, installing and
  configuring. The UX should be intuitive; where help or explanation is
  needed, keep it next to the widget in question through descriptive labels,
  tooltip cards and visual indicators rather than prose in the docs.

## Upstream docs

- [iced book](https://book.iced.rs/) and [API docs](https://docs.iced.rs/)
- [iced_layershell](https://github.com/waycrate/exwlshelleventloop) (and the
  fork carrying the active patch)
- [niri IPC](https://github.com/YaLTeR/niri) for the compositor protocol
- [zbus](https://dbus2.github.io/zbus/) for D-Bus data sources

`README.md` covers running, installing and configuring the shell.
