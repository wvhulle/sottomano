# Contributing

Conventions and workflow for working on odin-rs. Read this before
touching the code.

## Workflow

- **Verify visually on every UI change.** Run alongside Odin (the
  reference bar) — "it compiles" is not a green light. **Do not
  screenshot; it is unreliable here.** Instead, ask the user to
  visually inspect the top strip against Odin's rendering and tell you
  what they see.
- **Parity first.** Match Odin's behaviour before improving it. Note
  intentional deviations when you make them.
- **Keep the bar building & runnable at every step.** Small,
  shippable commits; never land a commit that breaks the build.

## Manual verification (the feedback loop)

There is a live niri session with the **original Odin bar running**.
odin-rs is designed to run *alongside* it and render **flush below
odin's top bar** (it sets `exclusive_zone: 0`, so niri drops it below
odin's reserved top zone automatically).

odin-rs shows an orange marker so it's unmistakable next to the
Odin bar. Ask the user to visually inspect its clock / battery /
workspaces against Odin's own bar and report back — do not attempt
to screenshot.

What to check on each pass:

1. **Anchored top bar + exclusive zone** — maximize a terminal; its
   top edge should sit below the original Odin bar (niri drops it
   under odin's reserved zone). Niri keyboard shortcuts and terminal
   input must work on the first keypress — the bar never steals focus.
1. **Data sources** — battery / cpu / temperature values update as
   the sources poll. Break one source (e.g. make a sysfs file
   unreadable) and confirm the clock keeps ticking and the last value
   is retained.
1. **Live niri workspaces** — switch workspaces in niri; the active
   pill should follow within a frame. There should be no `niri msg`
   subprocess running (it's the event stream, not polling).

Kill a running instance with `pkill -f target/debug/odin-rs` (⚠ don't
put that exact string in a larger shell command — pkill will match
its own wrapper).

## Comments

Keep comments to an absolute minimum. Well-named functions, types, and variables
should make code self-documenting. Only add a comment when the *why* is non-obvious
to future maintainers—typically explaining a subtle invariant, hidden constraint,
or workaround for a specific bug.

**Don't write comments that:**
- Explain what code does (`// increment counter` on `count += 1`)
- Reference other code, functions, fields, or modules
- Discuss implementation status, phases, or future work
- Mention issues, callers, or prior versions
- Explain design tradeoffs without being load-bearing (belongs in commit message)

**Avoid multi-line docstrings.** One short line max if necessary.


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
