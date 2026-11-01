# Guide

Every group that has a recording and is not already in the [README](../../README.md), rendered from fixtures, so the numbers are made up. The shell also has groups for the clock, disks, GPU, network interfaces, power, the system tray, systemd units and a few more, which look and behave the same way.

Each page opens with its first recording in every [look](appearance.md), side by side.

## Group menus

Most groups live in menus behind a single icon on a panel, so the panels stay short. Hovering the icon opens the menu: one row per group, with settings that share a unit lined up in a table above them. Hovering a row opens the group's items in a panel beside it, and hovering an item there shows its card. Menus open toward the side of the screen with more room, downward from the top panel and upward from the bottom panel.

The pin on a row puts that group back on the panel, next to its menu icon; a pin on a row inside a group's submenu puts only that reading there. The unpin button in its card returns it to the menu. Pins are remembered across restarts. A group whose alert reaches `panel_bar.group_menu_promotion_severity` also shows on the panel until the alert clears.

| Menu | Panel | Groups |
|---|---|---|
| Hardware | top, left | power, compute, memory and board, GPUs, storage, drives |
| Network | top, left | interfaces, ethernet ports, Bluetooth, reachability, VPN |
| Agenda | top, middle | calendar, weather, quiet schedule, focus session |
| Media | top, right | media players, audio, dictation, game controllers |
| Appearance | bottom, left | panel, sidebar, overlay and window settings with a unit |
| Behaviour | bottom, left | the remaining panel, sidebar, overlay and window settings |
| Display | bottom, left | monitors, dark mode, colour temperature, wallpaper |
| Services | bottom, right | local models, AI usage, systemd units, journal, upgrades |
| Session | bottom, right | idle timeouts, login session |

## Top panel

The panel I look at most: the Hardware and Network menus on the left; the clock, Agenda menu and notifications in the middle; the Media menu and the power menu on the right.

- [Compute](compute.md): load, clock, temperature and caches.
- [Memory and board](memory-and-board.md): memory use, modules and firmware.
- [Weather](weather.md): location, wind, rain and pollen.
- [Audio and media](audio-and-media.md): the playing track, volume and microphone.
- [More of the top panel](top-panel-extras.md): VPN, dictation, agenda and game controllers.

## Bottom panel

Where the windows live: the Appearance, Behaviour and Display menus, the looks and the workspaces on the left; the windows in the middle; the Services and Session menus and the system tray on the right.

- [Windows and workspaces](windows-and-workspaces.md): previews, the window list and the process inspector.
- [Wallpaper](wallpaper.md): the place in the wallpaper and its article.
- [Idle and session](idle-and-session.md): idle timeouts and the login session.
- [AI usage](ai-usage.md): subscription windows, billing block and spend.
- [Appearance](appearance.md): the looks and how to switch them.

## Notification toasts

- [Notification toasts](toasts.md): arrival, progress and actions.
