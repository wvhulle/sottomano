# Contributing

Conventions and workflow for working on odin-rs. Read this before
touching the code.

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
