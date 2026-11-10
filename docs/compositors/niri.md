# niri

Sottomano talks to [niri](https://github.com/YaLTeR/niri) through `niri-ipc`
26.4, so use niri 26.04 or newer. Everything the bar offers works on niri,
including the overview, column moves and the workspace order restored from
niri's config. For installing niri itself, follow
[its instructions](https://github.com/YaLTeR/niri).

## Start with the session

niri's `niri-session` starts `graphical-session.target`, which starts the
sottomano user service from [the session page](../session.md).

The bar's frames only reach the screen through niri, so give niri at least the
bar's CPU weight in `~/.config/systemd/user/niri.service.d/sottomano-priority.conf`,
which takes effect at the next login:

```ini
[Service]
CPUWeight=1000
```

The home-manager module writes this file when `services.niri.enable` is set.

## Window styling rules

The window styling group writes `~/.config/sottomano/niri.kdl`: blur for the
panel and toasts, shadows behind the panels, window corner radius, gaps, and the
switch-heat borders. Include it in `~/.config/niri/config.kdl`:

```kdl
include "/home/alice/.config/sottomano/niri.kdl" optional=true
```

The home-manager module adds this include when `services.niri.enable` is set.
niri reloads the file after each change.
