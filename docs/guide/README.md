# Guide

Every group that has a recording and is not already in the [introduction](../introduction.md), rendered from fixtures, so the numbers are made up. The shell also has groups for the clock, disks, GPU, network interfaces, power, the system tray, systemd units and a few more, which look and behave the same way.

Widgets that look alike share one recording, with the others listed in a table below it. The [compute](compute.md) page shows a group in its menu next to the same group pinned in the bar. The [looks](appearance.md) have a page of their own.

## Group menus

Most groups live in menus behind a single icon on a panel. Pressing the icon opens the menu: one row per group, with settings that share a unit lined up in a table above them. Hovering a row opens the group's items in a panel beside it, pressing it opens them at once, and hovering an item's info icon there shows its card. A menu stays open until a press anywhere outside it, on any monitor, a press on its icon or Escape; while it is open, hovering or pressing another menu icon switches to that menu. Menus open downward from the top panel and upward from the bottom panel.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/group_menu_media.webp" width="902" alt="Opening the Media menu, pressing its rows and hovering the microphone">

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/group_menu_appearance.webp" width="892" alt="Opening the Appearance menu with its settings lined up in a table">

The pin on a row puts that group back on the panel, next to its menu icon; a pin on one of the group's items puts only that item there. The unpin button in its card returns it to the menu. Pins are remembered across restarts. An item shown in red (its severity reaches `group_menus.alert_promotion_severity`, `critical` by default) also shows on the panel until it clears; dismiss it from its card to hide it until the next alert.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/group_menu_pin.webp" width="564" alt="Pinning the memory group from the Hardware menu, which moves it onto the panel">

The Menus group in the Behaviour menu changes how every menu opens. Menus can open from their icon on hover instead of on press, and such a menu closes when the pointer leaves it. In an accordion, pressing a row opens the group's items below it, and several groups can stay open.

<img src="https://codeberg.org/wvhulle/sottomano/raw/branch/recordings/group_menu_accordion_several.webp" width="443" alt="Opening two groups of the Hardware menu as an accordion">

| Menu | Panel | Groups |
|---|---|---|
| Hardware | top, left | power, compute, memory and board, GPUs, storage, drives |
| Network | top, left | Wi-Fi adapters, mobile modems, ethernet ports, Bluetooth, reachability, VPN |
| Application menu | top, left | the focused window's menus, one group per top-level menu; off until switched on in Behaviour |
| Agenda | top, middle | calendar, weather, quiet schedule, focus session |
| Display | top, right | monitors, dark mode, colour temperature, wallpaper |
| Media | top, right | media players, audio, dictation, game controllers |
| Appearance | bottom, left | panel, sidebar, overlay, window and menu settings with a unit |
| Behaviour | bottom, left | the remaining panel, sidebar, overlay, window and menu settings, the application menu switch, looks |
| Applications | bottom, middle | frequently launched apps, one group per category, launcher options |
| Virtualization | bottom, right | virtual machines, microVMs, Incus instances, nspawn containers, LXC containers, Podman containers, Docker containers, Waydroid, Flatpak apps, memory limits |
| Services | bottom, right | local models, CI runs, AI usage, systemd units, journal, upgrades |
| Session | bottom, right | idle timeouts, login session |

## Top panel

The Hardware, Network and Application menus on the left; the clock, Agenda menu and notifications in the middle; the Display and Media menus and the power menu on the right.

- [Compute](compute.md): load, clock, temperature and caches.
- [Memory and board](memory-and-board.md): memory use, modules and firmware.
- [Weather](weather.md): location, wind, rain and pollen.
- [Audio and media](audio-and-media.md): the playing track, volume and microphone.
- [More of the top panel](top-panel-extras.md): VPN, dictation, agenda and game controllers.
- [Wallpaper](wallpaper.md): the place in the wallpaper and its article.

## Bottom panel

The Appearance and Behaviour menus and the workspaces on the left; the windows in the middle; the Virtualization, Services and Session menus and the system tray on the right.

- [Windows and workspaces](windows-and-workspaces.md): previews, the window list and the process inspector.
- [Idle and session](idle-and-session.md): idle timeouts and the login session.
- [AI usage](ai-usage.md): subscription windows, billing block and spend.
- [Network](network.md): WireGuard tunnels with a switch to connect or disconnect them, and who may reach this machine over Tailscale.
- [Virtualization and CI runs](virtualization.md): running QEMU, libvirt and microVM guests, Incus instances, systemd-nspawn and LXC containers, the containers in each Podman and Docker engine, Waydroid and Flatpak sandboxes, memory caps on slices and services, and the runs of local CI runners.
- [Appearance](appearance.md): the looks and how to switch them.

## Notification toasts

- [Notification toasts](toasts.md): arrival, progress and actions.
