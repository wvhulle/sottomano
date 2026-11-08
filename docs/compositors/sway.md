# sway

Sottomano follows [sway](https://swaywm.org) through its IPC socket
(`SWAYSOCK`). It is tested with sway 1.12.

## Install sway

On NixOS, set `programs.sway.enable = true`. Ubuntu and Fedora package sway:

```bash
sudo apt install sway
sudo dnf install sway
```

## Start with the session

sway does not start `graphical-session.target` by itself, which the sottomano
user service from [the installation guide](../installation.md#start-with-the-session)
waits for. Start a target bound to it at the end of your sway config, after
exporting the session environment to systemd:

```text
exec dbus-update-activation-environment --systemd WAYLAND_DISPLAY SWAYSOCK XDG_CURRENT_DESKTOP && systemctl --user start sway-session.target
```

with `~/.config/systemd/user/sway-session.target`:

```ini
[Unit]
BindsTo=graphical-session.target
Wants=graphical-session.target
After=graphical-session-pre.target
```

## Window styling rules

The window styling group writes `~/.config/sottomano/sway.conf` with the window
gaps. Include it in your sway config:

```text
include ~/.config/sottomano/sway.conf
```

The home-manager module adds this include when
`wayland.windowManager.sway.enable` is set. Sottomano asks sway to reload after
each change.

## What sway does not offer

- Stock sway has no rounded window corners, blur or per-window border colours,
  so the window styling group only shows the gaps slider.
- There is no overview: pressing the active workspace does nothing.
- Workspaces cannot be reordered: dragging one in the bar reports that the
  compositor has no equivalent. Dragging a window onto another workspace moves
  it, but windows have no columns to reorder within a workspace.
- Toggling the overview, maximizing, centring and focusing the previous window
  from a gamepad report that sway has no equivalent.

Apps launched from the bar start in their own transient user service through
`systemd-run --user`.
