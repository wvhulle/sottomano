# Contributing to sottomano-shell

The shell crate composes the widgets into the running `sottomano` binary. The
workspace-wide conventions live in the root `CONTRIBUTING.md`; this file covers
what only applies here.

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

The revision is the one the iced fork's devtools pins (`COMPATIBLE_REVISION` in
`iced_devtools/src/comet.rs`); bump it when the fork moves. The shell's beacon
keeps retrying its connection, so comet and the shell can start in either order.
Comet then shows each message with its update, view, layout and draw timings,
plus task and subscription counts; under `time-travel` it can rewind the shell
to an earlier message.
