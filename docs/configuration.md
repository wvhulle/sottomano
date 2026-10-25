# Configuration

There is one TOML file, and the shell prints its complete default itself, so
there is no option reference here to keep in sync. This page shows how the file
is read and gives examples of the settings people change most.

## The config file

The file is `$XDG_CONFIG_HOME/sottomano.toml`, usually
`~/.config/sottomano.toml`. Every key is optional, so the file only needs what
differs from the defaults. To start from the full default:

```bash
sottomano --print-default-config > ~/.config/sottomano.toml
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

## Secrets

A few widgets need an API key or password. Each one is optional, and the
widgets that need a missing one stay hidden.

```bash
sottomano --list-secrets
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
