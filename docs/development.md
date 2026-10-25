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

```mermaid
flowchart TB
  subgraph sources_crate [sottomano-sources]
    desktop_bus["D-Bus (zbus)<br>power, login session, media, …"]
    notification_server["freedesktop<br>notification server"]
    niri_event_thread["niri IPC<br>event thread"]
    wayland_outputs["Wayland outputs"]
    file_watches["inotify<br>config file, desktop entries"]
    reading_sources["ReadingSource::delivery_stream"]
  end

  subgraph config_crate [sottomano-config]
    config_file[("$XDG_CONFIG_HOME/sottomano.toml")]
    config_loading["ConfigFile::load"]
    config_file --> config_loading
  end

  subgraph widgets_crate [sottomano-widgets]
    source_subscription["background_work::source_subscription"]
    shell_message["Message (shell_message.rs)"]
    source_subscription --> shell_message
  end

  subgraph shell_crate [sottomano-shell]
    run_shell["run_shell"]
    message_subscriptions["message_subscriptions::subscription"]
    shell_update["DesktopShell::update"]
    shell_view["DesktopShell::view"]
    layer_surfaces["layer-shell surfaces<br>bars, panel, sidebar, toasts, tooltips"]
    run_shell -->|startup load tasks| shell_update
    message_subscriptions --> shell_update
    shell_update -->|state| shell_view
    shell_view --> layer_surfaces
  end

  subgraph state_crate [sottomano-state]
    saved_state_modules["dark_mode, launches, idle,<br>focus_spans, niri_rules, …"]
  end

  sqlite_state_store[("$XDG_STATE_HOME/sottomano/state.db<br>via sqlite_state_store")]

  config_loading --> run_shell
  desktop_bus & notification_server & niri_event_thread & wayland_outputs & file_watches --> message_subscriptions
  reading_sources -->|SourceDelivery| source_subscription
  shell_message --> message_subscriptions
  shell_update <-->|restore / persist| saved_state_modules
  saved_state_modules <--> sqlite_state_store
```

## Building and checking

- `cargo r` for a debug run.
- `cargo clippy --workspace --all-targets` before committing. The workspace
  warns on clippy pedantic plus `absolute_paths`.
- `treefmt` formats Rust (rustfmt, nightly for the options in
  `.rustfmt.toml`) and Nix (nixfmt), as configured in `treefmt.toml`.
- `cargo xtask update-config-docs` regenerates the full default config in
  `docs/configuration.md`; with `--check` it only reports whether it is stale.

Version control is jj, not the `git` CLI.

Debugging the running shell with iced's devtools and comet is described in
[sottomano-shell/CONTRIBUTING.md](../sottomano-shell/CONTRIBUTING.md).

## CI

Woodpecker CI (`.woodpecker/`) builds the flake `checks` (tests, clippy and
format) on pull requests and on `main`. It also checks pull request commits
against Conventional Commits
(locally: `nix run --inputs-from . nixpkgs#convco -- check main..HEAD`). After
checks pass on `main`, the `readme-recordings` pipeline re-renders the tour and
opens a pull request when the recordings changed. The `config-docs` pipeline
does the same for the full default config in `docs/configuration.md` when the
config crate changes.

```mermaid
flowchart LR
  pull_request_event(["pull request"])
  main_push_event(["push or manual on main"])

  subgraph checks_pipeline [".woodpecker/checks.yaml"]
    cargo_checks_pr["cargo-checks-pr<br>tests, clippy"]
    cargo_checks["cargo-checks<br>tests, clippy via cachix"]
    format_check["format<br>treefmt: rustfmt, nixfmt"]
    conventional_commits["conventional-commits<br>convco check FETCH_HEAD..HEAD"]
  end

  subgraph recordings_pipeline [".woodpecker/readme-recordings.yaml"]
    render_recordings["render-readme-recordings<br>nix build .#readme-recordings"]
    push_recordings_branch["push-readme-recordings-branch<br>force-push to Codeberg"]
    open_recordings_pull_request["open-readme-recordings-pr<br>open, update or close"]
    render_recordings -->|"cp result/*.webp recordings/"| push_recordings_branch
    push_recordings_branch --> open_recordings_pull_request
  end

  subgraph config_docs_pipeline [".woodpecker/config-docs.yaml"]
    update_config_docs["update-config-docs<br>cargo xtask update-config-docs"]
    push_config_docs_branch["push-config-docs-branch<br>force-push to Codeberg"]
    open_config_docs_pull_request["open-config-docs-pr<br>open, update or close"]
    update_config_docs --> push_config_docs_branch
    push_config_docs_branch --> open_config_docs_pull_request
  end

  main_branch[("main")]

  pull_request_event --> cargo_checks_pr & format_check & conventional_commits
  main_push_event --> cargo_checks & format_check
  checks_pipeline -->|"depends_on: checks"| recordings_pipeline
  checks_pipeline -->|"depends_on: checks"| config_docs_pipeline
  open_recordings_pull_request -->|readme-recordings PR| main_branch
  open_config_docs_pull_request -->|config-docs PR| main_branch
```

The `readme-recordings` pipeline skips pushes that only touch `recordings/**`
and its own commits. The `config-docs` pipeline only runs when
`sottomano-config/`, `xtask/` or `docs/configuration.md` change, and skips its
own commits.

## Tour recordings

The recordings in [showcase.md](showcase.md) and the README are rendered by
`sottomano-widget-showcase`.
[sottomano-widget-showcase/CONTRIBUTING.md](../sottomano-widget-showcase/CONTRIBUTING.md)
describes how they are scripted, added and checked.

```mermaid
flowchart LR
  subgraph widget_showcase [sottomano-widget-showcase]
    group_fixtures["fixtures/*.rs<br>group(Clock), live(Clock)<br>fixed showcase_epoch"]
    demo_script["demo_script::DEMOS<br>Hover, Press, Drag, RestFor, …"]
    showcase_program["ShowcaseProgram::scripted"]
    animation_recording["animation_recording::record<br>iced_test emulator, frame clock,<br>ContentCrop, lavapipe"]
    embedding_check["tour_document_embedding::assert_embedded"]
    interactive_tester["ShowcaseProgram::interactive<br>--window"]
    ice_scenarios["tests/ice_scenarios.rs<br>.ice scenarios"]
    group_fixtures --> demo_script --> showcase_program --> animation_recording
    group_fixtures --> interactive_tester
    group_fixtures --> ice_scenarios
    animation_recording -->|name, width| embedding_check
  end

  recorded_webp[/"recordings/&lt;name&gt;.webp"/]
  showcase_document["docs/showcase.md<br>must embed every recording"]
  readme_document["README.md<br>checks the recordings it embeds"]

  animation_recording -->|lossless animated webp| recorded_webp
  embedding_check -->|"&lt;img src width&gt;"| showcase_document
  embedding_check -->|"&lt;img src width&gt;"| readme_document
```

## Upstream docs

- [iced book](https://book.iced.rs/) and [API docs](https://docs.iced.rs/)
- [exwlshelleventloop](https://github.com/waycrate/exwlshelleventloop), the
  source of `iced_exwlshell` (and the fork carrying the active patches)
- [niri IPC](https://github.com/YaLTeR/niri) for the compositor protocol
- [zbus](https://dbus2.github.io/zbus/) for D-Bus data sources
