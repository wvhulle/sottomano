# Configuration

There is one TOML file. The complete default below is generated from the code,
so it cannot drift from what the shell actually reads.

## The config file

The file is `$XDG_CONFIG_HOME/sottomano.toml`, usually
`~/.config/sottomano.toml`. Every key is optional, so the file only needs what
differs from the defaults. `sottomano default-config` prints the
[full default](#full-default), and its output is a working starting point for
the file.

The running shell watches the file and applies every saved change in place.
Unknown keys are rejected. An invalid file raises a toast naming the problem,
and the previous configuration stays in use.

With home-manager, `services.sottomano.settings` takes the same keys as Nix
attributes, and the module writes them to that file verbatim.

## Full default

Every key with its default, as `sottomano default-config` prints it.

<!-- default-config:start -->
```toml
[theme.palette]
background = "#0b0e14"
text = "#e6e1cf"
primary = "#95e6cb"
success = "#aad94c"
warning = "#ffb454"
danger = "#f07178"
background_weak = "#202229"
background_strong = "#3e4b59"
secondary = "#bfbdb6"

[theme.font]
text_size = "14px"

[theme.spacing]
item_gap = "4px"
surface_inset = "10px"
group_gap = "14px"

[desktop_panels]
height = "24px"

[desktop_panels.slot_drag]
ghost_scale = 1.08
spring_load_delay = "600ms"
focus_indicator_duration = "160ms"

[group_menus]
alert_promotion_severity = "warning"

[widget_group_layout]
expansion_duration = "90ms"

[group_items]
slider_expansion_duration = "90ms"

[hover_cards]
copy_flash_duration = "700ms"

[hover_cards.loading]
debug_delay = "0s"

[hover_cards.loading.placeholder]
effect = "pulse"
period = "1s 400ms"

[hover_cards.loading.intro]
effect = "fade"
duration = "500ms"

[hover_cards.loading.replace]
effect = "decode"
duration = "350ms"
glyph_change = "55ms"

[overlay_surfaces]
surface_morph_duration = "160ms"
typing_idle_delay = "750ms"
typing_background_opacity = 0.3

[widget_groups.window]
audio_indicator = true
progress = true
count_badge = true
nixpkgs_branch = "nixpkgs-unstable"
maximum_window_title_width = "180px"

[widget_groups.window.agents]
kitty = true

[widget_groups.workspaces]
preview_scale = 0.19

[widget_groups.tray.items.Signal_status_icon_1]
hide_icon = false
left_click = "focus_app_window"
middle_click = "secondary_activate_item"
right_click = "ignore"
window_app_id = "signal"

[widget_groups.tray.items.spotify-client]
hide_icon = false
left_click = "focus_app_window"
middle_click = "secondary_activate_item"
right_click = "ignore"
window_app_id = "spotify"

[widget_groups.compute.temperature_sensors]

[widget_groups.physical_drive.temperature_sensors]

[widget_groups.systemd_manager]
slow_job = "10s"
slow_boot_unit = "5s"
slow_shutdown_stop = "5s"
slower_boot_warning_percent = 25
slower_boot_critical_percent = 60

[widget_groups.claude_usage]
monthly_budget_usd = 200

[widget_groups.copilot_usage]
monthly_budget_usd = 40

[widget_groups.openrouter_credit]
monthly_budget_usd = 20
daily_warning_usd = 5
max_blended_usd_per_million = 4.0

[widget_groups.session_focus]
idle_detection_delay = "30s"
shortest_counted_focus = "1s"
idle_after = "3m"
session_warning = "45m"
session_critical = "1h 30m"
warning_snooze = "15m"
switches_per_hour_warning = 100
switches_per_hour_critical = 300
switch_heat_span = "30m"
switch_heat_warning = 20
switch_heat_critical = 32

[widget_groups.session_focus.app_daily_budget_minutes]

[widget_groups.session_focus.project_daily_budget_minutes]

[widget_groups.weather.rain_alerts]
lead_time = "15m"
minimum_millimeters_per_hour = 0.5
heavy_millimeters_per_hour = 4.0

[widget_groups.reachability]
latency_target = "1.1.1.1"
metered_cycle_start_day = 1

[widget_groups.gamepad]
passive_app_id_prefixes = ["steam", "gamescope"]

[widget_groups.gamepad.navigation]
stick = "left"
up = "focus_workspace_up"
down = "focus_workspace_down"
left = "focus_column_left"
right = "focus_column_right"

[widget_groups.gamepad.pointer]
stick = "right"
pixels_per_second = 1600
left_button = "right_trigger"
right_button = "right_stick"

[widget_groups.gamepad.typed_keys]
south = "Return"
east = "BackSpace"
north = "shift+Tab"
west = "Escape"
dpad_up = "Up"
dpad_down = "Down"
dpad_left = "Left"
dpad_right = "Right"
back = "Tab"

[widget_groups.gamepad.compositor_actions]
left_shoulder = "move_column_left"
right_shoulder = "move_column_right"
start = "maximize_column"
guide = "toggle_overview"
left_stick = "focus_window_previous"

[widget_groups.gamepad.commands]

[widget_groups.gamepad.push_to_talk]
button = "left_trigger"

[widget_groups.caldav]
server_url = ""
user = ""
upcoming_days = 7
change_poll_interval = "1m"
push_safety_recheck_interval = "15m"
subscription_download_interval = "15m"

[widget_groups.media.lyrics]
lrclib_lookup = true

[notification_toasts.placement]
fallback_corner = "bottom-right"

[notification_toasts.lifetime]
low_severity_timeout = "8s"
normal_severity_timeout = "12s"
critical_severity_timeout = "0s"

[notification_toasts.card]
appear = "90ms"
dismiss_fade = "140ms"
expire_fade = "220ms"
motion = "160ms"

[calendar]
other_time_zones = ["Asia/Shanghai"]

[calendar.working_hours]
start = "09:00:00"
end = "17:00:00"

[launched_programs]
terminal_command_prefix = ["kitty", "-e"]
color_temperature_service = "screen-temperature.service"
```
<!-- default-config:end -->

## Secrets

A few widgets need an API key or password. Each one is optional, and the
widgets that need a missing one stay hidden. `sottomano secrets` lists the
secret files in `~/.config/sottomano/`, what each one enables and whether it is
present.

With home-manager, `services.sottomano.secretFiles.<name>` symlinks each one
from wherever the secret actually lives, so nothing lands in the Nix store.
