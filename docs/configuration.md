# Configuration

There is one TOML file, and the shell prints its complete default itself, so
there is no option reference here to keep in sync. This page shows how the file
is read and gives examples of the settings people change most.

## The config file

The file is `$XDG_CONFIG_HOME/sottomano.toml`, usually
`~/.config/sottomano.toml`. Every key is optional, so the file only needs what
differs from the defaults. To start from the full default, which is also listed under
[Full default](#full-default):

```bash
sottomano default-config > ~/.config/sottomano.toml
```

The running shell watches the file and applies every saved change in place.
Unknown keys are rejected. An invalid file raises a toast naming the problem,
and the previous configuration stays in use.

With home-manager, set `services.sottomano.settings` and the module writes it to
the same file verbatim. The two forms below are equivalent:

```toml
[calendar]
other_time_zones = ["America/New_York", "Asia/Tokyo"]

[widget_groups.rain_alerts]
lead_time = "30m"
```

```nix
services.sottomano.settings = {
  calendar.other_time_zones = [ "America/New_York" "Asia/Tokyo" ];
  widget_groups.rain_alerts.lead_time = "30m";
};
```

## Values

- Sizes are strings with a unit, either `px` or `em` relative to the text size:
  `"24px"`, `"0.5em"`. A bare number is rejected.
- Durations are strings such as `"90ms"`, `"45m"`, `"1h 30m"` or `"1s 400ms"`.
- Colours are hex strings: `"#0b0e14"`.

## Sections

| Section | Controls |
| --- | --- |
| `theme.palette`, `theme.font` | Colours and text and icon sizes |
| `layout.panel`, `layout.overlay` | Panel height, spacing inside panels and popups |
| `panel_bar` | Slider width, drag behaviour, expansion animations |
| `tooltips` | Hover card width and loading animations |
| `widget_groups.*` | Settings per widget group |
| `notification_toasts` | Toast placement, timeouts and card size |
| `calendar` | Extra time zones and working hours |
| `launched_programs` | Terminal and services the bar launches |

## Examples

### Theme

```toml
[theme.palette]
surface = "#1e1e2e"
bar_foreground = "#cdd6f4"
focus = "#94e2d5"
urgent = "#f38ba8"

[theme.font]
text_size = "15px"
icon_size = "16px"

[layout.panel]
height = "28px"
```

With the home-manager module and umbra, the palette follows the dark or light
mode instead. Keys set in `theme.palette` still override umbra's colours.

### Temperature sensors

Sensors are detected automatically and thresholds come from the hardware, so
these tables are empty by default. To pin a sensor or override a threshold:

```toml
[widget_groups.temperature_sensors.cpu]
sensor = { chip = "k10temp", label = "Tctl" }
warning_celsius = 80
critical_celsius = 90

[widget_groups.temperature_sensors.drive]
sensor = { chip = "drivetemp" }
critical_celsius = 60
```

The chip and label are the `name` and `temp*_label` files under
`/sys/class/hwmon/hwmon*/`.

### Focus time

Daily budgets are keyed by the window's app id or the project directory:

```toml
[widget_groups.focus_time]
session_warning = "50m"
screen_daily_budget_minutes = 480

[widget_groups.focus_time.app_daily_budget_minutes]
firefox = 120

[widget_groups.focus_time.project_daily_budget_minutes]
"/home/alice/Code/sottomano" = 240
```

### Tray

Each tray item is keyed by its StatusNotifierItem id. Clicks take
`activate_item`, `secondary_activate_item`, `focus_app_window` or `ignore`:

```toml
[widget_groups.tray.items.spotify-client]
left_click = "focus_app_window"
window_app_id = "spotify"

[widget_groups.tray.items.nm-applet]
hide_icon = true
```

### Calendar and CalDAV

```toml
[calendar]
other_time_zones = ["Europe/Brussels"]

[calendar.working_hours]
start = "08:30:00"
end = "16:30:00"

[widget_groups.caldav]
server_url = "https://cloud.example.org/remote.php/dav"
user = "alice"
upcoming_days = 14
```

The CalDAV group stays hidden until `server_url`, `user` and the
`caldav-password` secret are all set.

### Usage budgets

The AI usage pills colour their spend against a monthly budget:

```toml
[widget_groups.claude_usage]
monthly_budget_usd = 100

[widget_groups.openrouter_credit]
monthly_budget_usd = 10
daily_warning_usd = 2
```

### Gamepad

Buttons map to niri actions, typed keys or commands. Windows whose app id
starts with a passive prefix get the gamepad to themselves:

```toml
[widget_groups.gamepad]
passive_app_id_prefixes = ["steam", "gamescope", "retroarch"]

[widget_groups.gamepad.compositor_actions]
west = "close_window"

[widget_groups.gamepad.commands]
right_stick = ["notify-send", "gamepad", "right stick pressed"]
```

### Notification toasts

```toml
[notification_toasts.placement]
fallback_corner = "top-right"

[notification_toasts.lifetime]
normal_severity_timeout = "6s"
critical_severity_timeout = "0s"
```

A timeout of `"0s"` keeps the toast until it is dismissed.

### Launched programs

```toml
[launched_programs]
terminal_command_prefix = ["foot", "-e"]
color_temperature_service = "wlsunset.service"
```

## Full default

Every key with its default, as `sottomano default-config` prints it. CI
regenerates this section from the code, so do not edit it by hand.

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
widgets that need a missing one stay hidden.

```bash
sottomano secrets
```

This lists each secret file, what it enables and whether it is present. The
files live in `~/.config/sottomano/` and hold the secret as plain text:

| File | Enables |
| --- | --- |
| `openrouter-management-key` | OpenRouter spend (a management key, not a regular API key) |
| `claude-code-oauth` | Claude usage windows, from `~/.claude/.credentials.json` |
| `gh-auth-token` | Copilot quota, from `gh auth token` |
| `unsplash-access-key` | Wallpaper caption and photographer (the Client-ID) |
| `caldav-password` | CalDAV tasks and events |

Without home-manager, write or link them by hand:

```bash
mkdir -p ~/.config/sottomano
gh auth token > ~/.config/sottomano/gh-auth-token
chmod 600 ~/.config/sottomano/gh-auth-token
ln -s ~/.claude/.credentials.json ~/.config/sottomano/claude-code-oauth
```

With home-manager, `services.sottomano.secretFiles.<name>` symlinks each one
from wherever the secret actually lives, so nothing lands in the Nix store:

```nix
services.sottomano.secretFiles = {
  gh-auth-token = config.age.secrets.gh-auth-token.path;
  caldav-password = config.sops.secrets.caldav-password.path;
};
```
