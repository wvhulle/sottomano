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
surface = "#0b0e14"
bar_foreground = "#e6e1cf"
inactive = "#bfbdb6"
urgent = "#f07178"
warning = "#ffb454"
focus = "#95e6cb"
success = "#aad94c"
info = "#59c2ff"
dim = "#3e4b59"
surface_raised = "#202229"
frame_outline = "#3e4b59"
secondary_foreground = "#bfbdb6"

[theme.font]
text_size = "14px"
small_text_size = "12px"
icon_size = "15.5px"

[layout.panel]
height = "24px"
group_padding = "5px"
widget_spacing = "4px"
group_spacing = "11px"

[layout.overlay]
panel_gap = "14px"
content_inset = "10px"
row_spacing = "4px"

[panel_bar]
slider_width = "60px"
slider_stroke = "2px"
indicator_thickness = "2.4px"
frame_outline = "1px"
drag_start_distance = "6px"
drag_ghost_scale = 1.08
drag_spring_load = "600ms"
group_expansion = "90ms"
slider_expansion = "90ms"
focus_indicator_motion = "160ms"

[tooltips]
value_width = "620px"
popup_morph = "160ms"
copy_flash = "700ms"

[tooltips.loading]
debug_delay = "0s"

[tooltips.loading.placeholder]
effect = "pulse"
period = "1s 400ms"

[tooltips.loading.intro]
effect = "fade"
duration = "500ms"

[tooltips.loading.replace]
effect = "decode"
duration = "350ms"
glyph_change = "55ms"

[widget_groups.window_list]
audio_indicator = true
progress = true
count_badge = true
nixpkgs_branch = "nixpkgs-unstable"

[widget_groups.window_list.agents]
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

[widget_groups.temperature_sensors.cpu]

[widget_groups.temperature_sensors.drive]

[widget_groups.systemd_manager]
slow_job = "10s"
slow_boot_unit = "5s"
slow_shutdown_stop = "5s"

[widget_groups.claude_usage]
monthly_budget_usd = 200

[widget_groups.copilot_usage]
monthly_budget_usd = 40

[widget_groups.openrouter_credit]
monthly_budget_usd = 20
daily_warning_usd = 5

[widget_groups.focus_time]
idle_detection_delay = "30s"
shortest_counted_focus = "1s"
idle_after = "3m"
session_warning = "45m"
session_critical = "1h 30m"
break_grace = "5m"
switches_per_hour_warning = 100
switches_per_hour_critical = 300
switch_heat_span = "30m"
switch_heat_warning = 20
switch_heat_critical = 32

[widget_groups.focus_time.app_daily_budget_minutes]

[widget_groups.focus_time.project_daily_budget_minutes]

[widget_groups.rain_alerts]
lead_time = "15m"
minimum_millimeters_per_hour = 0.5
heavy_millimeters_per_hour = 4.0

[widget_groups.gamepad]
passive_app_id_prefixes = ["steam", "gamescope"]
handover_chord = ["back", "start"]

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

[widget_groups.gamepad.typed_keys]
south = "Return"
east = "Escape"
dpad_up = "Up"
dpad_down = "Down"
dpad_left = "Left"
dpad_right = "Right"

[widget_groups.gamepad.compositor_actions]
north = "maximize_column"
left_shoulder = "move_column_left"
right_shoulder = "move_column_right"
guide = "toggle_overview"

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

[notification_toasts.placement]
fallback_corner = "bottom-right"
edge_gap = "24px"
window_right_offset = "12px"
window_top_offset = "48px"

[notification_toasts.lifetime]
low_severity_timeout = "8s"
normal_severity_timeout = "12s"
critical_severity_timeout = "0s"

[notification_toasts.card]
width = "440px"
icon_size = "44px"
app_badge_size = "20px"
image_preview_max_height = "180px"
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
