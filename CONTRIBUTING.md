# Contributing

Conventions and workflow for working on odin-rs. Read this before
touching the code.

## Workflow

- **Verify visually on every UI change.** Run alongside Odin (the
  reference bar), screenshot the top strip, and diff against Odin's
  rendering — "it compiles" is not a green light.
- **Parity first.** Match Odin's behaviour before improving it. Note
  intentional deviations when you make them.
- **Keep the bar building & runnable at every step.** Small,
  shippable commits; never land a commit that breaks the build.
- **Version control is [jujutsu](https://martinvonz.github.io/jj/),
  not git.** See the maintainer's global rules.

## Manual verification (the feedback loop)

There is a live niri session with the **original Odin bar running**.
odin-rs is designed to run *alongside* it and render **flush below
odin's top bar** (it sets `exclusive_zone: 0`, so niri drops it below
odin's reserved top zone automatically). This lets you screenshot
both bars stacked and visually diff odin-rs against the reference.

```sh
# 1. build
nix develop -c cargo build

# 2. run alongside the live Odin bar (background; keeps rendering)
WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/1000 \
  nix develop -c ./target/debug/odin-rs &

# 3. screenshot. grim uses logical coordinates — get output geometry first:
niri msg --json outputs        # find your output's logical geometry
grim -g "0,0 900x70" top.png   # top strip: odin bar + odin-rs below it

# 4. enlarge a region to read widgets, then view the PNG
magick top.png -resize 300% top-big.png
```

odin-rs shows an orange marker so it's unmistakable next to the
Odin bar. Compare its clock / battery / workspaces against Odin's
own bar in the same image.

What to check on each pass:

1. **Anchored top bar + exclusive zone** — maximize a terminal; its
   top edge should sit below the original Odin bar (niri drops it
   under odin's reserved zone). Niri keyboard shortcuts and terminal
   input must work on the first keypress — the bar never steals focus.
2. **Data sources** — battery / cpu / temperature values update as
   the sources poll. Break one source (e.g. make a sysfs file
   unreadable) and confirm the clock keeps ticking and the last value
   is retained.
3. **Live niri workspaces** — switch workspaces in niri; the active
   pill should follow within a frame. There should be no `niri msg`
   subprocess running (it's the event stream, not polling).

For a headless data-layer check (no display, no compositor):
`nix develop -c ./target/debug/odin-rs -- --smoke` runs the data
paths and prints parsed output — use when only a data source changed.

Kill a running instance with `pkill -f target/debug/odin-rs` (⚠ don't
put that exact string in a larger shell command — pkill will match
its own wrapper).

## Design preferences

- **Distrust always-on timers.** Prefer signal/event-driven updates
  and infrequent polls. Surface visible failure over silent retry
  loops.
- **`view()` must be cheap.** iced rebuilds the entire surface's
  widget tree synchronously on every update — no I/O, no filesystem,
  no heavy compute in `view()`. Resolve or memoise derived data in
  `update()` (or behind interior mutability) before the widget code
  reads it. When something visually lags, time each group's `view()`
  before touching the render loop; the stall is almost always work
  inside `view()`, not the compositor. See the
  [iced architecture docs](https://docs.iced.rs/) for the model.
- **Build the same widget tree shape across renders.** Never
  conditionally omit a widget to swap in a different one — that
  defeats iced's diffing. Use opacity / visibility / size changes
  to express "off" states.
- **One visual primitive per semantic concept.** If multiple places
  need the same visual (focus, urgent, hover, etc.), factor a single
  primitive and reuse it everywhere. Don't re-implement the same
  look inline at each call site.

## Upstream patches

When carrying an upstream patch:

1. Vendor it as a git dependency pointing at a fork (not a `[patch]`
   section — git deps work in Nix builds; relative path patches
   don't). Track a branch so the fix is rebasable.
2. Document the divergence with a comment in `Cargo.toml` linking to
   the upstream issue/PR.
3. Drive an upstream PR. Until the patch lands, the fork's branch
   must keep the fix.

## Project pointers

- `README.md` — build, launch, configure.
- `TODO.md` — current phase, deferred work, open questions, upstream
  patches to chase.
- The Odin repo (`../odin`) is the **behavioural source of truth**.
  When porting a widget or data source, match Odin's QML/Nu
  counterpart.
- Upstream docs you will read repeatedly:
  - [iced book](https://book.iced.rs/) and [API docs](https://docs.iced.rs/)
  - [iced_layershell](https://github.com/waycrate/exwlshelleventloop) (and the fork carrying the active patch)
  - [niri IPC](https://github.com/YaLTeR/niri) for the compositor protocol
  - [zbus](https://dbus2.github.io/zbus/) for D-Bus data sources
