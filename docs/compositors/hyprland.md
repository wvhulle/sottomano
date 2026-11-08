# Hyprland

Sottomano follows [Hyprland](https://hypr.land) through its request and event
sockets (`HYPRLAND_INSTANCE_SIGNATURE`). It sends its requests as Lua
dispatchers (`hl.dsp`) and writes a Lua rule file, so it needs Hyprland 0.56 or
newer with a Lua config (`hyprland.lua`). It is tested with Hyprland 0.56.2.

## Install Hyprland

On NixOS, set `programs.hyprland.enable = true`. For other distributions, follow
[Hyprland's installation guide](https://wiki.hypr.land/Getting-Started/Installation/).
Start Hyprland through `start-hyprland`.

## Start with the session

Hyprland does not start `graphical-session.target` by itself, which the
sottomano user service from
[the installation guide](../installation.md#start-with-the-session) waits for.
Start a target bound to it from your Lua config, after exporting the session
environment to systemd:

```lua
hl.on("hyprland.start", function()
    hl.exec_cmd("dbus-update-activation-environment --systemd WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP && systemctl --user start hyprland-session.target")
end)
```

with `~/.config/systemd/user/hyprland-session.target`:

```ini
[Unit]
BindsTo=graphical-session.target
Wants=graphical-session.target
After=graphical-session-pre.target
```

## Window styling rules

The window styling group writes `~/.config/sottomano/hyprland.lua`: gaps,
rounding, blur for the panel and toasts, and the switch-heat border colours.
Load it at the end of your `hyprland.lua`:

```lua
local sottomano_rules = os.getenv("HOME") .. "/.config/sottomano/hyprland.lua"
local sottomano_rules_file = io.open(sottomano_rules)
if sottomano_rules_file then
    sottomano_rules_file:close()
    dofile(sottomano_rules)
end
```

The home-manager module does not add this, because home-manager's Hyprland
module writes a `hyprland.conf`. Sottomano asks Hyprland to reload after each
change.

## What Hyprland does not offer

- There is no overview: pressing the active workspace does nothing, and
  toggling the overview from a gamepad reports that Hyprland has no equivalent.
- Workspaces cannot be reordered: dragging one in the bar reports that the
  compositor has no equivalent. Dragging a window onto another workspace moves
  it, but windows have no columns to reorder within a workspace.

Apps launched from the bar start in their own transient user service through
`systemd-run --user`.
