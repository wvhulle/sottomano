# Contributing

Conventions and workflow for working on vetrata.

## Workflow

Work inside the Nix devshell (`nix develop`). The repository is a cargo
workspace: `vetrata-shell/` holds the shell (binary `vetrata`) and
`vetrata-readme-showcase/` renders the README recordings. `cargo r` for a
debug run, `cargo clippy --workspace --all-targets` before committing — the
workspace warns on clippy pedantic plus `absolute_paths` — and `cargo fmt`, which
needs a nightly toolchain for the options in `.rustfmt.toml`. Version control is jj, not the `git` CLI.

## Conventions

- No comments, doc-comments or docstrings. Intent lives in names and call flow;
  code that cannot be made self-evident that way is restructured, not annotated.
- Strict config/state separation: no field appears in both the TOML config
  (`vetrata-shell/src/config/`) and runtime state (`vetrata-shell/src/state/`).
- No `macro_rules!` deduplication. Use traits, methods and marker types.
- No global config statics. Config is threaded through as instance data.
- Widgets over free helper functions: style functions, builder widgets and
  methods on the owning type (`vetrata-shell/src/widgets/`,
  `vetrata-shell/src/widget_groups/`).
- Widget state lives in its widget and routes through its group's `Action` —
  no `App`-level carve-outs, no shadow copies of a cell's state.
- Names describe structure, not appearance.
- No user instructions in `README.md` beyond running, installing,
  configuring and the recorded tour with one-line captions. The UX should be
  intuitive; where help or explanation is needed, keep it next to the widget
  in question through descriptive labels, tooltip cards and visual indicators
  rather than prose in the docs.
- No config keys, defaults or TOML snippets in `README.md`. Users get a
  complete default config from `vetrata --print-default-config` and start
  from there, so every option must show up in that output with its default.

## README recordings

The tour recordings in `recordings/` are rendered offscreen by the
`vetrata-readme-showcase` crate. Each entry of `DEMOS` in `demo_script.rs` names a
fixture and a script of `Hover` / `Press` steps on `HoverTarget`s; the
recorder finds each target on screen, plays the steps through `iced_test`'s
emulator and writes `recordings/<name>.webp`, a lossless animated WebP cropped to what
the demo shows.

To add a demo, write `fixtures/<group>.rs` (the group fed its readings through
its own messages), give the group a variant in `showcased_group.rs` if it has
none yet, and add a `Demo` entry. A target that is not on screen fails the run
and lists the targets that are.

```bash
cargo run -p vetrata-readme-showcase
cargo run -p vetrata-readme-showcase -- weather
```

The full run also checks that `README.md` embeds every recording as
`<img src="recordings/<name>.webp" width="…">` at half its pixel width, so all demos
show at the same scale; it prints the expected tag when one is off.

## Upstream docs

- [iced book](https://book.iced.rs/) and [API docs](https://docs.iced.rs/)
- [iced_layershell](https://github.com/waycrate/exwlshelleventloop) (and the
  fork carrying the active patch)
- [niri IPC](https://github.com/YaLTeR/niri) for the compositor protocol
- [zbus](https://dbus2.github.io/zbus/) for D-Bus data sources

`README.md` covers running, installing and configuring the shell.
