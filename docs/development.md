# Development

Setup and workflow for working on sottomano. The rules the code follows are in
[CONTRIBUTING.md](../CONTRIBUTING.md).

## Workspace

Work inside the Nix devshell (`nix develop`). The repository is a cargo
workspace:

- `sottomano-config/` holds the TOML config types and their loading.
- `sottomano-sources/` holds the data sources and sinks the shell reads and
  writes.
- `sottomano-state/` holds the runtime state the shell persists and restores.
- `sottomano-widgets/` holds the widgets, widget groups, hover tooltips and the
  `Message` they produce.
- `sottomano-shell/` composes them into the running shell (binary `sottomano`).
- `sottomano-widget-showcase/` plays widget fixtures for the tour recordings,
  the `.ice` scenarios and the windowed tester.

## Runtime

`run_shell` loads the config and starts the `DesktopShell` daemon. Every data
source becomes a subscription that yields a `Message`. `update` applies it,
persisting state where needed, and `view` renders the layer-shell surfaces.

<img src="diagrams/runtime_data_flow.svg" alt="Runtime data flow from config, sources and saved state through update and view to the layer-shell surfaces">

## Building and checking

- `cargo r` for a debug run.
- `cargo clippy --workspace --all-targets` before committing. The workspace
  warns on clippy pedantic plus `absolute_paths`.
- `treefmt` formats Rust (rustfmt, nightly for the options in
  `.rustfmt.toml`), Nix (nixfmt) and D2 (`d2 fmt`), as configured in
  `treefmt.toml`.

Version control is jj, not the `git` CLI.

Debugging the running shell with iced's devtools and comet is described in
[sottomano-shell/CONTRIBUTING.md](../sottomano-shell/CONTRIBUTING.md).

## CI

Woodpecker CI (`.woodpecker/`) builds the flake `checks` (tests, clippy,
format and diagrams) on pull requests and on `main`. It also checks pull request commits
against Conventional Commits
(locally: `nix run --inputs-from . nixpkgs#convco -- check main..HEAD`). After
checks pass on `main`, the `readme-recordings` pipeline re-renders the tour and
opens a pull request when the recordings changed.

<img src="diagrams/ci_pipelines.svg" alt="Woodpecker pipelines: checks on pull requests and main, then readme-recordings opening a pull request into main">

## Diagrams

The diagrams in this document are [D2](https://d2lang.com) sources in
`docs/diagrams/` with the ELK layout. Rendered SVGs are committed next to them
because forges do not render D2. After editing a `.d2` file, re-render it with
`d2 docs/diagrams/<name>.d2 docs/diagrams/<name>.svg`. The `diagrams` flake
check fails when a committed SVG is stale.

## Tour recordings

The recordings in [showcase.md](showcase.md) and the README are rendered by
`sottomano-widget-showcase`.
[sottomano-widget-showcase/CONTRIBUTING.md](../sottomano-widget-showcase/CONTRIBUTING.md)
describes how they are scripted, added and checked.

<img src="diagrams/tour_recording_pipeline.svg" alt="Tour recording pipeline from fixtures and demo scripts to webp recordings and the embedding check">

## Upstream docs

- [iced book](https://book.iced.rs/) and [API docs](https://docs.iced.rs/)
- [exwlshelleventloop](https://github.com/waycrate/exwlshelleventloop), the
  source of `iced_exwlshell` (and the fork carrying the active patches)
- [niri IPC](https://github.com/YaLTeR/niri) for the compositor protocol
- [zbus](https://dbus2.github.io/zbus/) for D-Bus data sources
