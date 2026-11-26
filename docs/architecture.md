# Architecture

How sottomano's crates fit together and how the running shell works, one
diagram per flow. The workflow for building, debugging and testing it is in
[Development](development.md).

## Workspace

The repository is a cargo workspace:

- `sottomano-toml-config/` holds the TOML config types and their loading.
- `sottomano-compositor-model/` holds the compositor-neutral window, workspace,
  output, request and styling types, and the `CompositorBackend` trait every
  compositor implements.
- `sottomano-niri/`, `sottomano-hyprland/` and `sottomano-sway/` each implement
  `CompositorBackend` for one compositor: its IPC events and queries, its
  requests, its window styling rules and whatever only it supports. All
  compositor-specific code lives in these three crates.
- `sottomano-compositors/` detects the running compositor and dispatches to its
  backend. Other crates stay compositor-neutral by construction: none of them
  depends on a backend crate or an IPC library, `CompositorKind` keeps its
  variants private, and the `compositor-feature-cfg` ast-grep rule leaves the
  compositor features to this crate's dispatch.
- `sottomano-desktop-sources/` holds the data sources the shell reads, as
  streams, together with the bus proxies and connection handles they share.
- `sottomano-desktop-sinks/` holds what the shell writes: commands, settings and
  signals sent to desktop services, built on the handles from the sources crate.
- `corredo/` holds generic iced widgets that know nothing about the shell.
- `sottomano-widgets/` holds the panels, menus, groups, indicators and popups,
  laid out by the hierarchy in [Style](style.md#vocabulary).
  Each indicator group restores and persists its own state.
- `sottomano-shell/` composes them into the running shell (binary `sottomano`).
  The widgets crate defines the interactions its groups emit; the shell and the
  scenes crate each wrap them in their own message type.
- `sottomano-widget-scenes/` holds the widget fixtures and scenes, and the
  scripted session that plays steps on them, for the windowed tester, the UI
  regression tests and the `.ice` scenarios in `sottomano-widgets/tests/`.
- `sottomano-tour-recordings/` renders the demos of the guide and the README.

## Startup

The shell loads the config and starts its daemon. A data source becomes a
subscription that yields a message while a reading it backs is on screen or can
alert, decided the same way for pinned groups and groups in a menu. Windows and
workspaces come from whichever compositor runs (niri, sway or Hyprland), detected
by `sottomano-compositors` from its socket variable; each compositor's backend
crate sits behind a cargo feature of the same name, all on by default.

```mermaid
flowchart TB
  config_file[("$XDG_CONFIG_HOME/sottomano.toml")]
  config_loading["config loading"]
  config_file --> config_loading

  subgraph compositors_crate [sottomano-compositors]
    compositor_events["compositor IPC<br>niri, sway or Hyprland backend"]
  end

  subgraph sources_crate [sottomano-desktop-sources]
    file_watches["inotify<br>config file, desktop entries"]
  end

  subgraph shell_crate [sottomano-shell]
    shell_startup["shell startup"]
    message_subscriptions["message subscriptions"]
    shell_update["shell update"]
  end

  config_loading --> shell_startup
  shell_startup -->|startup load tasks| shell_update
  compositor_events & file_watches --> message_subscriptions
  message_subscriptions --> shell_update
```

## Readings to messages

Every live reading arrives as a message from a subscription: D-Bus services,
the notification server, compositor events, Wayland outputs and file watches.
Snapshot sources instead yield whole widgets at once, for the tour recordings
and the windowed tester.

```mermaid
flowchart TB
  subgraph sources_crate [sottomano-desktop-sources]
    desktop_bus["D-Bus (zbus)<br>power, login session, media, …"]
    notification_server["freedesktop<br>notification server"]
    wayland_outputs["Wayland outputs"]
    file_watches["inotify<br>config file, desktop entries"]
    snapshot_sources["snapshot sources"]
  end

  subgraph compositors_crate [sottomano-compositors]
    compositor_events["compositor IPC<br>niri, sway or Hyprland backend"]
  end

  message_subscriptions["message subscriptions"]
  snapshot_subscription["snapshot subscription"]
  shell_message["shell messages"]

  desktop_bus & notification_server & compositor_events & wayland_outputs & file_watches --> message_subscriptions
  snapshot_sources -->|snapshots| snapshot_subscription
  snapshot_subscription --> shell_message
  shell_message --> message_subscriptions
```

## Messages to screen and back

The update step applies each message, persisting state where needed, and the
view renders the layer-shell surfaces. Each indicator group updates and views its
own items, restoring and persisting its state in the per-group SQLite database.

```mermaid
flowchart TB
  shell_update["shell update"]
  shell_view["shell view"]
  app_launch_store["app launch history"]

  subgraph widgets_crate [sottomano-widgets]
    indicator_groups["indicator groups<br>update, view"]
    group_stores["per-group SQLite state<br>dark mode schedule, idle timeouts,<br>focus spans, shell settings, …"]
    indicator_groups <-->|restore / persist| group_stores
  end

  layer_surfaces["layer-shell surfaces<br>bars, panel, sidebar, toasts, hover cards"]
  state_database[("$XDG_STATE_HOME/sottomano/state.db")]

  shell_update <-->|group messages and tasks| indicator_groups
  shell_update -->|state| shell_view
  shell_view --> layer_surfaces
  group_stores & app_launch_store <--> state_database
  shell_update <-->|restore / persist| app_launch_store
```

## Writes to the desktop

The write tasks the indicator groups produce go through the sinks crate: service
commands, notification signals and the window styling rule file, which each
[compositor's page](compositors/) includes in its config. The running
compositor's backend renders the rules and carries out compositor requests.

```mermaid
flowchart TB
  indicator_groups["indicator groups"]

  subgraph sinks_crate [sottomano-desktop-sinks]
    service_commands["service commands<br>volume, power, network, tray, …"]
    notification_signals["notification<br>signals"]
    rules_file_writes["window styling<br>rule file writes"]
  end

  subgraph compositors_crate [sottomano-compositors]
    compositor_requests["compositor requests<br>and rule rendering"]
  end

  compositor_rules_file[("$XDG_CONFIG_HOME/sottomano/<br>niri.kdl, sway.conf or hyprland.lua")]

  indicator_groups -->|write tasks| service_commands & notification_signals & rules_file_writes & compositor_requests
  rules_file_writes -->|rendered by| compositor_requests
  rules_file_writes -->|rewrite| compositor_rules_file
```

The CI pipelines and the tour recordings each have a diagram of their own in
[Development](development.md).