# Showcase

Every group that has a recording and is not already in the [README](../README.md), rendered from fixtures, so the numbers are made up. The shell also has groups for the clock, disks, GPU, network interfaces, power, the system tray, systemd units and a few more, which look and behave the same way.

## Top panel

The panel I look at most: the machine on the left, the day in the middle, sound and input on the right. Each group sits folded to a few glyphs and expands on hover or click.

### Compute: CPU

Pressing the CPU reading unfolds load, temperature, power and model.

<img src="../recordings/compute_cpu.webp" width="679" alt="Expanding the compute group from the CPU reading">

### Compute: Model

The CPU model with its vendor logo, microarchitecture and caches.

<img src="../recordings/compute_model.webp" width="679" alt="Hovering the CPU model in the expanded compute group">

### Compute: Temperature

The hottest sensor, and the chip it is read from.

<img src="../recordings/compute_temperature.webp" width="679" alt="Hovering temperature in the expanded compute group">

### Compute: Clock

Each thread's clock per chiplet, with the cores the scheduler prefers starred.

<img src="../recordings/compute_clock.webp" width="679" alt="Hovering the CPU clock in the expanded compute group">

### Compute: Cache preference

On dual-chiplet X3D parts, which chiplet the scheduler fills first; a click flips it.

<img src="../recordings/compute_cache_preference.webp" width="685" alt="Hovering the X3D cache preference in the expanded compute group">

### Memory and board: Memory

Memory use, the processes holding the most and the installed modules.

<img src="../recordings/memory_board_memory.webp" width="679" alt="Expanding the memory and board group from the memory reading">

### Memory and board: Board

The board with its maker's logo, model and firmware.

<img src="../recordings/memory_board_board.webp" width="679" alt="Hovering the board in the expanded memory and board group">

### Weather: Location

How rough the location fix is and which source produced it.

<img src="../recordings/weather_location.webp" width="517" alt="Hovering the location in the weather group">

### Weather: Wind

Wind speed, gusts and direction, with the Beaufort force in the panel.

<img src="../recordings/weather_wind.webp" width="401" alt="Hovering the wind in the weather group">

### Weather: Rain

Rain in the coming hours on one timeline, split by hour: radar every five minutes where Buienradar covers, the forecast model's precipitation type every quarter-hour everywhere.

<img src="../recordings/weather_precipitation.webp" width="401" alt="Hovering the rain item in the weather group">

### Weather: Pollen

Today's pollen per allergen, so hay fever has a forecast too.

<img src="../recordings/weather_pollen.webp" width="401" alt="Hovering the pollen in the weather group">

### VPN: WireGuard

A WireGuard client tunnel and the server this machine runs, with each peer's last handshake and traffic.

<img src="../recordings/vpn_wireguard.webp" width="556" alt="Expanding the VPN group and hovering a WireGuard tunnel">

### Dictation: Pipeline

What the dictation daemon is doing right now and how fast the last pass decoded.

<img src="../recordings/voice_dictation_pipeline.webp" width="366" alt="Expanding the dictation group and hovering the pipeline">

### Agenda: Events and tasks

Upcoming events and open tasks over CalDAV, overdue tasks in red.

<img src="../recordings/caldav_agenda.webp" width="673" alt="Expanding the agenda and hovering its events and tasks">

### Media: Track

The playing track from any MPRIS player, with where to find it.

<img src="../recordings/media_track.webp" width="406" alt="Hovering the playing track">

### Audio: Volume

Clicking the speaker opens a slider in place; dragging sets the level.

<img src="../recordings/audio_volume.webp" width="423" alt="Opening the volume slider and dragging it">

### Audio: Microphone

The microphone with its live level meter, straight from PipeWire.

<img src="../recordings/audio_microphone.webp" width="423" alt="Expanding the audio group and hovering the microphone">

### Game controller: Bindings

The controller drawn with every bound button labelled in place.

<img src="../recordings/gamepad_controller.webp" width="669" alt="Hovering the game controller to see its bindings drawn on the controller">

## Bottom panel

Where the windows live, with the desktop's settings on the left and the software running in the background on the right.

### Workspaces: Previews

Hovering a workspace shows its windows at a glance.

<img src="../recordings/bottom_workspaces.webp" width="515" alt="Hovering a workspace in the bottom panel">

### Windows: Window

A window's title, application and workspace.

<img src="../recordings/bottom_window.webp" width="515" alt="Hovering a window in the bottom panel">

### Windows: Process inspector

The process tree behind a window, to see what a terminal is really running.

<img src="../recordings/bottom_process_inspector.webp" width="720" alt="Hovering the process inspector of a window">

### Wallpaper: Place

The place in the current wallpaper, when it carries a location.

<img src="../recordings/wallpaper_place.webp" width="433" alt="Expanding the wallpaper group from the place lookup">

### Wallpaper: Article

The Wikipedia summary for that place.

<img src="../recordings/wallpaper_article.webp" width="715" alt="Hovering the Wikipedia article in the wallpaper group">

### Idle timeouts: Screen off

Dragging a timeout changes it; the card says what happens when it fires.

<img src="../recordings/idle_screen_off.webp" width="319" alt="Expanding the idle timeouts and hovering the screen-off timer">

### Idle timeouts: Lock

The lock timer, next to the screen-off timer so the two read together.

<img src="../recordings/idle_lock.webp" width="267" alt="Expanding the idle timeouts and hovering the lock timer">

### AI usage: Usage windows

How much of each subscription window is used, and where it is heading by reset.

<img src="../recordings/ai_usage_windows.webp" width="442" alt="Expanding the AI usage group and hovering the Claude usage windows">

### AI usage: Billing block

The current five-hour block: spend so far, burn rate and where it will end.

<img src="../recordings/ai_usage_block.webp" width="442" alt="Expanding the AI usage group and hovering the billing block">

### AI usage: Periods

Spend today, this week and this month against the monthly budget.

<img src="../recordings/ai_usage_periods.webp" width="551" alt="Expanding the AI usage group and hovering the spend periods">

### Login session: Kernel and compositor

Distribution, kernel and compositor, each with its logo and a wiki link.

<img src="../recordings/login_session_kernel.webp" width="679" alt="Expanding the login session group and hovering the kernel and compositor">

### Workspace sidebar: Previews

The same workspace previews, turned vertical for wide screens.

<img src="../recordings/sidebar_workspaces.webp" width="426" alt="Hovering each workspace in the vertical sidebar">

## Notification toasts

The shell is the notification daemon, so there is nothing else to run.

### Toast: Arrival

A toast arrives, holds while hovered and expires on its own.

<img src="../recordings/toast_arrival.webp" width="489" alt="A toast arriving, holding while hovered and expiring">

### Toast: Progress

A download that reports progress draws a bar instead of repeating itself.

<img src="../recordings/toast_progress.webp" width="489" alt="A download toast with a rising progress bar">

### Toast: Actions

Actions the sender offers appear as buttons.

<img src="../recordings/toast_actions.webp" width="489" alt="Hovering the actions of an invitation toast and accepting it">
