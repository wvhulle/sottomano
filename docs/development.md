# Development

Setup and workflow for working on sottomano: building and checking it,
debugging, CI, the VM tests and the tour recordings. How the crates fit
together is in [Architecture](architecture.md); the rules the code follows are
in [Style](style.md).

Work inside the Nix devshell (`nix develop`).

## Building and checking

- `cargo r` for a debug run.
- `cargo clippy --workspace --all-targets` before committing. The workspace
  warns on clippy pedantic plus `absolute_paths`.
- `treefmt` formats Rust (rustfmt, nightly for the options in
  `.rustfmt.toml`) and Nix (nixfmt), as configured in `treefmt.toml`.
- `cargo xtask update-config-docs` regenerates the full default config in
  `docs/configuration.md`; with `--check` it only reports whether it is stale.

Version control is jj, not the `git` CLI.

## Debugging with comet

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

The revision is the one the iced fork's devtools pins; bump it when the fork
moves. The shell's beacon keeps retrying its connection, so comet and the shell
can start in either order. Comet then shows each message with its update, view,
layout and draw timings, plus task and subscription counts; under `time-travel`
it can rewind the shell to an earlier message.

## CI

Woodpecker CI (`.woodpecker/`) builds the flake `checks` (tests, clippy and
format) on pull requests and on `main`. It also checks pull request commits
against Conventional Commits
(locally: `nix run --inputs-from . nixpkgs#convco -- check main..HEAD`). After
checks pass on `main`, the recordings pipeline re-renders the tour, force-pushes
it as the single commit of the orphan `recordings` branch and opens a pull
request when the embed widths changed. The config docs pipeline does the same
for the full default config in `docs/configuration.md` when the config crate
changes.

```mermaid
flowchart LR
  pull_request_event(["pull request"])
  main_push_event(["push or manual on main"])

  subgraph checks_pipeline ["checks pipeline"]
    pull_request_checks["tests, clippy"]
    main_checks["tests, clippy,<br>then push to the binary cache"]
    format_check["format<br>treefmt: rustfmt, nixfmt"]
    conventional_commits["conventional commits<br>convco check"]
  end

  subgraph recordings_pipeline ["recordings pipeline"]
    render_recordings["render the tour<br>nix build .#readme-recordings<br>cargo xtask update-recording-embeds"]
    push_orphan_recordings["replace the single commit<br>of the recordings branch"]
    push_recordings_branch["force-push resized embeds<br>to Codeberg"]
    open_recordings_pull_request["open, update or close<br>the pull request"]
    render_recordings -->|"recordings/*.webp"| push_orphan_recordings
    render_recordings -->|resized embeds| push_recordings_branch
    push_recordings_branch --> open_recordings_pull_request
  end

  subgraph config_docs_pipeline ["config docs pipeline"]
    update_config_docs["regenerate the default config<br>cargo xtask update-config-docs"]
    push_config_docs_branch["force-push to Codeberg"]
    open_config_docs_pull_request["open, update or close<br>the pull request"]
    update_config_docs --> push_config_docs_branch
    push_config_docs_branch --> open_config_docs_pull_request
  end

  main_branch[("main")]

  pull_request_event --> pull_request_checks & format_check & conventional_commits
  main_push_event --> main_checks & format_check
  checks_pipeline -->|"after checks"| recordings_pipeline
  checks_pipeline -->|"after checks"| config_docs_pipeline
  open_recordings_pull_request -->|recordings PR| main_branch
  open_config_docs_pull_request -->|config docs PR| main_branch
```

The recordings pipeline skips its own commits. The config docs pipeline only runs
when the config crate, `xtask/` or `docs/configuration.md` change, and skips its
own commits.

## VM tests

`vm-tests/` boots a NixOS VM per compositor (niri, sway, Hyprland) and scenario,
runs sottomano in a real session there and checks it through the compositor's
IPC and OCR of the screen. Each check is a flake check named
`vm-<compositor>-<scenario>`:

- `panel-mapped`: the bar starts and maps its panel.
- `styling-rules`: the bar writes the running compositor's rule file and the
  compositor accepts it.
- `workspace-follow`: named windows on two workspaces show in the bar after
  each workspace switch.

```bash
nix build .#checks.x86_64-linux.vm-sway-workspace-follow
nix run .#checks.x86_64-linux.vm-sway-workspace-follow.driverInteractive
```

The VMs need KVM and render in software. The niri in the VMs is patched to
accept a software renderer. The bar there uses a large font and a
high-contrast palette, so OCR reads it. A failing check prints the compositor's
windows and workspaces and the user journal.

The Woodpecker pipelines skip the VM tests. A Forgejo Actions workflow
(`.forgejo/workflows/vm-tests.yaml`) runs them on pushes to `main` on a runner
labelled `nix` with KVM.

## Tour recordings

The recordings in the [guide](guide/README.md) and the README are rendered
offscreen by `sottomano-widget-showcase` into the untracked root `recordings/`.
`nix build .#readme-recordings` renders them all the way CI does. Its source is
only the crates the showcase links, the workspace manifests and `.cargo/`;
`sottomano-shell` and `xtask` contribute their `Cargo.toml` (which takes part in
workspace feature unification) and empty targets. Changes to the shell, xtask,
the docs, the showcase's `.ice` tests or nix code that does not define the
recordings keep the cached recordings.

```bash
cargo run -p sottomano-widget-showcase
cargo run -p sottomano-widget-showcase -- --jobs 2
cargo run -p sottomano-widget-showcase -- <demo-name>
cargo run -p sottomano-widget-showcase -- --window
```

Rendering every demo runs one child process per demo, `--jobs` at a time (4 by
default). A demo buffers its frames until it encodes them, so lower `--jobs` when
memory is short.

```mermaid
flowchart LR
  subgraph widget_showcase [sottomano-widget-showcase]
    group_fixtures["group fixtures<br>on a fixed scripted clock"]
    demo_scripts["demo scripts<br>hover, press, drag, rest, …"]
    scripted_program["scripted program"]
    animation_recorder["recorder<br>iced_test emulator, frame clock,<br>content crop, lavapipe"]
    interactive_tester["interactive tester<br>--window"]
    ice_scenarios[".ice scenarios"]
    group_fixtures --> demo_scripts --> scripted_program --> animation_recorder
    group_fixtures --> interactive_tester
    group_fixtures --> ice_scenarios
  end

  embedding_update["cargo xtask update-recording-embeds"]

  recorded_webp[/"recordings/&lt;name&gt;.webp"/]
  showcase_document["docs/guide/*.md<br>embeds the rest, widths rewritten"]
  readme_document["README.md<br>widths rewritten for the recordings it embeds"]

  animation_recorder -->|lossless animated webp| recorded_webp
  recorded_webp -->|pixel width / 2| embedding_update
  embedding_update -->|"&lt;img src width&gt;"| showcase_document
  embedding_update -->|"&lt;img src width&gt;"| readme_document
```

### Scripting a demo

Each demo shows one widget, is named `<group>_<widget>` and sits under its
heading on a page of `docs/guide/`, unless the README shows it. The name also
selects the iced preset that boots its fixture for the recorder, the `.ice`
scenarios and the windowed tester alike.

A demo names a fixture and a script of steps: hovering, pressing or dragging
named targets for panel widgets, and hovering or pressing visible text for
toasts, which a toast fixture serves when the demo starts. The recorder finds
each target on screen, plays the steps through `iced_test`'s emulator and writes
`recordings/<name>.webp`, a lossless animated WebP cropped in both directions to
what the demo shows, plus the panel gap as a margin. A group whose handle
previews one of its items is opened by pressing that item's target. A handle
without a preview is opened through the target for the group's face.

To add a demo, write a fixture for the group (fed its readings through its own
messages), add the group to the showcase's groups if it is not there yet, and
add the demo to the list of demos. A target that is not on screen fails the run
and lists the targets that are.

A demo can put several fixtures in one menu. The settings groups are shown with
their messages dropped, because those groups save to SQLite on update. Pressing
the pin of a menu row only changes the showcase's in-memory state.

### Reproducibility

Rendering twice gives identical files, which
`nix build .#readme-recordings --rebuild` verifies after a first build. Time is
data: every preset boots with a clock whose wall time is the fixed showcase
epoch, and the recorder advances that clock one frame interval per captured frame
and sends it as a message. The frame also rewrites the redraw instant iced hands
to widgets, so animations run from the same scripted time. Live readings, toast
expiry and the hover card hide timer fire when their deadline passes on that
clock, and the scripted program has no subscriptions.

Keep that property when writing fixtures and widgets: take the clock as an
argument, never read the system time (the widgets crate's clippy config forbids
it, also inside macros such as `vec!`), derive a fixture's sample index from the
clock, and return deadlines instead of starting timers. iced's own text input
blinks its cursor from the real clock, so no demo types into one.

### Windowed tester

`--window` opens the presets in a real window with the iced tester attached,
which switches between presets and records interactions as `.ice` scenarios.
Scenarios saved in the showcase crate's tests run with `cargo test` (and so in
CI); `expect` sees plain text widgets, not rich text such as toast bodies.

### Embedding recordings

The recordings are not part of `main`: CI publishes them as the single commit of
the orphan `recordings` branch, so the history of `main` stays free of media.
That branch serves the recordings when the markdown is read on Codeberg; the
documentation site serves its own copy (see below).
Each recording appears either in the root `README.md` (a few highlights) or on
a page of `docs/guide/` (the rest), as
`<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/<name>.webp" width="<w>">`.
`<w>` is the recording's cropped width in logical pixels, so all demos show at
the same scale. After rendering, `cargo xtask update-recording-embeds` rewrites
`<w>` from the files in `recordings/`; it fails with the expected tag only when a
recording is embedded nowhere. CI runs it right after the render.

Rendering every widget takes long, so each visually distinct kind of widget has
one demo, and widgets that look alike are listed in a table on the guide page
below it. The compute page opens with a two-column table: the group's menu demo
next to its pinned demo, which boots with that group pinned. Both images carry
`alt` before `width="100%"`, so the width rewrite leaves them alone and each fills
half the page. The canvas is transparent, so clear and translucent panels show
the page behind them.

## Documentation site

`docs/` is an mdBook published at [sottomano.work](https://sottomano.work); its
first page includes the root `README.md`. The mermaid diagrams render through
the `mdbook-mermaid` preprocessor, whose `install` subcommand adds the bundled
`mermaid.min.js` and `mermaid-init.js` to the book and the
`[preprocessor.mermaid]` entry to `book.toml` (the nix build runs it on a copy;
run it once yourself at the repo root before `mdbook serve`, which serves the
book from there too). `mdbook serve` previews it locally, and `nix build .#documentation-site` builds what the server hosts. The
site updates when the nuremberg host in nixos-config is rebuilt with a newer
sottomano input.

The build rewrites the Codeberg recording and logo URLs and the absolute
`https://sottomano.work/` links to site paths, and links `recordings/` to the
output of `.#readme-recordings`, so the pages and their recordings come from the
same commit. It then checks every link offline with lychee, heading anchors
included, and fails on a missing page, anchor or recording. Building the site
therefore needs `.#readme-recordings`: from the binary cache, or rendered on
the spot.

## Upstream docs

- [iced book](https://book.iced.rs/) and [API docs](https://docs.iced.rs/)
- [exwlshelleventloop](https://github.com/waycrate/exwlshelleventloop), the
  source of `iced_exwlshell` (and the fork carrying the active patches)
- [niri IPC](https://github.com/YaLTeR/niri) for the niri protocol
- [sway-ipc(7)](https://man.archlinux.org/man/sway-ipc.7) for the sway protocol
- [Hyprland IPC](https://wiki.hypr.land/IPC/) and its Lua dispatchers for Hyprland
- [zbus](https://z-galaxy.github.io/zbus/) for D-Bus data sources
