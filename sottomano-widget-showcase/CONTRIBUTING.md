# Contributing to sottomano-widget-showcase

This crate plays widget fixtures for the tour recordings, the `.ice`
scenarios and the windowed tester. The workspace-wide conventions live in the
root `CONTRIBUTING.md`; this file covers what only applies here.

## Tour recordings

The tour recordings are rendered offscreen by this crate into the untracked
root `recordings/` (`nix build .#readme-recordings` renders them all the way CI does). Each
entry of `DEMOS` in `src/demo_script.rs` shows one widget, is named
`<group>_<widget>` and sits under its heading on a page of `docs/guide/`,
unless the README shows it. The name is also the demo's iced `Preset`, which boots its
fixture for the recorder, the `.ice` scenarios and the windowed tester alike. It
names a fixture and a script of steps: `Hover` / `Press` / `Drag` on
`HoverTarget`s for panel widgets, and `HoverText` / `PressText` on
visible text for toasts, which a `ShowcasedGroup::Toasts` fixture serves when
the demo starts. The recorder finds each target on screen, plays the steps
through `iced_test`'s emulator and writes `recordings/<name>.webp`, a lossless
animated WebP cropped in both directions to what the demo shows, plus the panel
gap as a margin. A group whose handle previews one of its items is opened by
pressing that item's target. A handle without a preview is opened with
`HoverTarget::GroupFace`.

Recordings are reproducible: rendering twice gives identical files, which
`nix build .#readme-recordings --check` verifies. Time is data: every preset
boots with a `sottomano_widgets::clock::Clock` whose wall time is the fixed
showcase epoch, and the recorder advances that clock one frame interval per
captured frame and sends it as a `Frame` message. The frame also rewrites the
redraw instant iced hands to widgets, so animations run from the same scripted
time. Live readings, toast expiry and the hover card hide timer fire when their
deadline passes on that clock, and the scripted program has no subscriptions.
Keep that property when writing fixtures and widgets: take the clock as an
argument, never read `Instant::now()` or `Utc::now()` (the widgets crate's
`clippy.toml` forbids them, also inside macros such as `vec!`), derive a
fixture's sample index with `fixtures::sample(clock)`, and return deadlines
instead of starting timers. iced's own `text_input` blinks its cursor from the
real clock, so no demo types into one.

To add a demo, write `src/fixtures/<group>.rs` (the group fed its readings
through its own messages), give the group a variant in `src/showcased_group.rs`
if it has none yet, and add a `Demo` entry. A target that is not on screen fails
the run and lists the targets that are.

`ShowcasedGroup::MenuMembers` puts several fixtures in one menu.
`ShowcasedGroup::ShellSettings` shows the settings groups with their messages
dropped, because those groups save to sqlite on update. `Step::PressPin` presses
the pin of a menu row, which only changes the showcase's in-memory state.

```bash
cargo run -p sottomano-widget-showcase
cargo run -p sottomano-widget-showcase -- --jobs 2
cargo run -p sottomano-widget-showcase -- weather_location
cargo run -p sottomano-widget-showcase -- --window
```

Rendering every demo runs one child process per demo, `--jobs` at a time (4 by
default). A demo buffers its frames until it encodes them, so lower `--jobs` when
memory is short.

`--window` opens the presets in a real window with the iced tester attached,
which switches between presets and records interactions as `.ice` scenarios.
Scenarios saved in `tests/ice/` run with `cargo test` (and so in CI); `expect`
sees plain text widgets, not rich text such as toast bodies.

The recordings are not part of `main`: CI publishes them as the single commit of
the orphan `recordings` branch, so the history of `main` stays free of media.
Each recording appears either in the root `README.md` (a few highlights) or on
a page of `docs/guide/` (the rest), as
`<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/<name>.webp" width="<w>">`. `<w>` is the recording's cropped
width in logical pixels, so all demos show at the same scale. After rendering,
`cargo xtask update-recording-embeds` rewrites `<w>` from the files in
`recordings/`; it fails with the expected tag only when a recording is embedded
nowhere. CI runs it right after the render.

Rendering every widget takes long, so each visually distinct kind of widget has
one demo, and widgets that look alike are listed in a table on the guide page
below it. The compute page opens with a two-column table: the group's menu demo
next to its `<name>_pinned` demo, whose `shown` is `GroupShown::PinnedFrom`, which
boots with that group pinned. Both images carry `alt` before `width="100%"`, so
the width rewrite leaves them alone and each fills half the page. The canvas is transparent, so clear and translucent panels
show the page behind them.
