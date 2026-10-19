# Contributing to sottomano-widget-showcase

This crate plays widget fixtures for the README recordings, the `.ice`
scenarios and the windowed tester. The workspace-wide conventions live in the
root `CONTRIBUTING.md`; this file covers what only applies here.

## README recordings

The tour recordings in the root `recordings/` are rendered offscreen by this
crate (`nix build .#readme-recordings` renders them all the way CI does). Each
entry of `DEMOS` in `src/demo_script.rs` shows one widget, is named
`<group>_<widget>` and sits in the root `README.md` under the heading of its
panel and group. The name is also the demo's iced `Preset`, which boots its
fixture for the recorder, the `.ice` scenarios and the windowed tester alike. It
names a fixture and a script of steps: `Hover` / `Press` / `Drag` on
`HoverTarget`s for panel widgets, and `HoverText` / `PressText` on
visible text for toasts, which a `ShowcasedGroup::Toasts` fixture serves when
the demo starts. The recorder finds each target on screen, plays the steps
through `iced_test`'s emulator and writes `recordings/<name>.webp`, a lossless
animated WebP that spans the full canvas width with its height cropped to what
the demo shows.

Recordings are reproducible: rendering twice gives identical files, which
`nix build .#readme-recordings --check` verifies. Time is data: every preset
boots with a `sottomano_widgets::clock::Clock` whose wall time is the fixed
showcase epoch, and the recorder advances that clock one frame interval per
captured frame and sends it as a `Frame` message. The frame also rewrites the
redraw instant iced hands to widgets, so animations run from the same scripted
time. Live readings, toast expiry and the tooltip hide timer fire when their
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

```bash
cargo run -p sottomano-widget-showcase
cargo run -p sottomano-widget-showcase -- weather_location
cargo run -p sottomano-widget-showcase -- --window
```

`--window` opens the presets in a real window with the iced tester attached,
which switches between presets and records interactions as `.ice` scenarios.
Scenarios saved in `tests/ice/` run with `cargo test` (and so in CI); `expect`
sees plain text widgets, not rich text such as toast bodies.

The full run also checks that the root `README.md` embeds every recording as
`<img src="recordings/<name>.webp" width="720">`, the logical canvas width, so
all demos show at the same scale; it prints the expected tag when one is off.
