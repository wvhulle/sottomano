# Guide

Each panel holds menus; a menu holds groups; a group holds indicators; hovering an indicator opens its popup, made of sections. The recordings use fixtures, so the numbers are made up.

## Levels

| Level | What it is |
|---|---|
| Panel | The bar along the top or bottom edge, split into a left, middle and right zone. |
| Menu | An icon on a panel that opens a list of groups. |
| Group | One row in a menu, for one source such as the CPU or Tailscale. |
| Instance group | A group repeated per device or item: per GPU, adapter, player, controller or game. |
| Indicator | One reading or control inside a group. |
| Popup | The card that opens when hovering an indicator or its info icon. |
| Section | A titled block in a popup. The last section, Actions, lists what each mouse button does. |

## Using menus

| Gesture | Where | Effect |
|---|---|---|
| Press | menu icon | Opens the menu; pressing it again closes it. |
| Hover or press | other menu icon while a menu is open | Switches to that menu. |
| Hover | group row | Opens the group's indicators beside the menu. |
| Press | group row | Opens the indicators at once, or below the row in an accordion. |
| Hover | indicator or info icon | Opens its popup. |
| Press outside, Escape | anywhere | Closes the menu. |
| Press | pin on a group row or indicator | Puts it on the panel next to its menu icon. |
| Press | unpin in a pinned popup | Returns it to the menu. |

Pins are remembered across restarts. An indicator whose severity reaches `group_menus.alert_promotion_severity` also shows on the panel until it clears or is dismissed from its popup. The Menus group in the [Settings](settings.md) menu switches to accordion menus and to opening menus on hover.

<img src="https://sottomano.work/recordings/group_menu_media.webp" srcset="https://sottomano.work/recordings/group_menu_media.webp 2x" alt="Opening the Media menu, pressing its rows and hovering the microphone">

<img src="https://sottomano.work/recordings/group_menu_pin.webp" srcset="https://sottomano.work/recordings/group_menu_pin.webp 2x" alt="Pinning the memory group from the Hardware menu, which moves it onto the panel">

<img src="https://sottomano.work/recordings/group_menu_accordion_several.webp" srcset="https://sottomano.work/recordings/group_menu_accordion_several.webp 2x" alt="Opening two groups of the Hardware menu as an accordion">

## Menus

| Menu | Panel | Zone | Groups |
|---|---|---|---|
| [Hardware](hardware.md) | top | left | power, compute, memory, GPUs, filesystems, drives |
| [Network](network.md) | top | left | Wi-Fi, mobile modems, ethernet, Bluetooth, reachability, Tailscale, WireGuard, VPN |
| [Menu bar](menu-bar.md) | top | left | the focused window's menus |
| [Agenda](agenda.md) | top | middle | clock, calendar, weather, location, quiet schedule, notifications, focus session |
| [Display](display.md) | top | right | monitors, dark mode, colour temperature, wallpaper |
| [Media](media.md) | top | right | players, audio, voice dictation |
| [Gaming](gaming.md) | top | right | game controllers, Steam downloads, Steam games, Steam tools |
| [Power](power.md) | top | right | lock, sleep, shutdown and session actions |
| [Applications](applications.md) | bottom | left | installed applications |
| [Settings](settings.md) | bottom | left | looks, panels, sidebar, overlays, windows, menus, menu bar, launcher |
| [Windows and workspaces](windows-and-workspaces.md) | bottom | left, middle | workspaces, window list |
| [Virtualization](virtualization.md) | bottom | right | virtual machines, containers, sandboxes, memory limits |
| [Services](services.md) | bottom | right | local models, CI runs, AI usage, systemd, journal, upgrades |
| [Session](session.md) | bottom | right | idle, login session, tray |

[Notification toasts](toasts.md) appear outside the panels.
