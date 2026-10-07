# Vetrata

My personal desktop shell for the [niri](https://github.com/YaLTeR/niri)
compositor: two panels in one binary, built on [iced](https://iced.rs) and
[iced_layershell](https://crates.io/crates/iced_layershell), reading D-Bus with
`zbus` where a service exists and calling the CLI where one does not.

## A tour

Both panels are rows of widget groups. A collapsed group is a single icon,
sometimes with its most important reading next to it and a thin gauge showing
how full it is. Click the icon and the group slides open to show every reading
it holds; click it again to fold it back. Hover any reading for a card with the
details behind it. The card grows out of the group you hovered, and sliding
along the bar morphs it into the next card instead of popping a new one. Values
in a card, such as an address, a PID or a network name, go to the clipboard
when clicked.

Open groups fold back when another window takes focus, unless you switch that
off in the panel settings group.

### Top panel, left: the machine

![The weather, battery, system, network and transfer groups expanded, the CPU card open](docs/top-bar.png)

From left to right, all expanded:

- **Weather.** Temperature, wind, when rain is due and where you are.
- **Battery.** Charge, power draw, time until empty or full, and the power
  profile.
- **Compute.** CPU load, memory, clock and temperature. Each CPU card lists every core,
  split into performance and efficient cores on hybrid CPUs: the load card
  breaks load into user, system, I/O wait, interrupts and deep-idle time, the
  clock card shows each core's clock next to its cap and warns when a cap
  holds a core below its hardware maximum, and the temperature card shows each
  core's temperature.
- **Disks.** Disk usage, read and write throughput, and drive health.
- **Network interfaces.** Ethernet, Bluetooth, Wi-Fi and the mobile modem, one
  reading per link.
- **Transfer.** Download and upload throughput, WireGuard tunnels and the
  Tailscale mesh with its peer count.
- **Panel settings.** The panel's own look and layout: float offset, tint,
  corner radius, drop shadow with its softness and opacity, whether groups are
  separate pills or one bar, whether they fold back on focus change, and
  whether the workspaces sit in the left sidebar or inline in the bottom
  panel.
- **Niri.** Blur and xray behind the panels. Each change rewrites
  `~/.config/vetrata/niri.kdl`, which niri includes and reloads live, so the
  panels restyle in place.

![The weather group expanded, the location card open](docs/location.png)

The location behind the weather comes from a Wi-Fi fix through BeaconDB. Its
card shows the place, coordinates, accuracy, and how many access points and
cells were seen and submitted.

### Top panel, right: you and the room

![The AI, monitor, media and volume groups expanded, the AI usage card open](docs/indicators.png)

- **AI usage.** Claude subscription, Copilot quota and OpenRouter spend, tracked
  while you work. The card shows session and weekly usage, when each resets,
  and extra spend against its limit.
- **Monitors.** Backlight of each display and whether automatic brightness is
  on.
- **Media.** The current MPRIS track, with previous, play/pause, stop and next,
  and a jump back to the app that owns it.
- **Audio.** Microphone and speaker; the speaker opens into a volume slider.

![The dark mode, colour temperature and idle timer groups expanded, the colour temperature slider open](docs/settings.png)

Settings adjust in place. Click a reading in an open group and it turns into a
slider in the bar itself.

- **Dark mode.** The current mode, whether it switches on its own, the time
  until the next switch, and offsets from sunrise and sunset.
- **Colour temperature.** Screen warmth in kelvin, a gamma dim, and whether both
  follow the sun.
- **Idle timers.** How long until the session locks, the screen turns off, the
  machine goes to standby and it powers down. The bolt beside each timer fires
  its action right away; the drastic ones ask for a second click first. Each
  card says when that timer will fire.

![The wallpaper group expanded, the Wikipedia card for where it was taken open](docs/wallpaper.png)

- **Wallpaper.** Where the current wallpaper was taken, its Wikipedia article,
  and the photographer. The article card carries the summary; the photo card
  links back to Unsplash.

### Left sidebar: workspaces

![The workspace group, the card for the hack workspace open](docs/workspaces.png)

A pill centred on the left edge, tinted like the panels, lists every workspace
by index and name, one below the other, with a mark beside each for the windows
on it. It grows to fit the longest name and reserves that width along the edge,
so windows tile beside it rather than under it. Short names keep it narrow,
such as a single emoji per workspace in the niri config. The focus marker slides to the new
workspace instead of blinking there. Hover a workspace for a card of its
windows. The sidebar toggle in the panel settings moves the workspaces back
into the bottom panel, next to the display icon, as a row.

### Bottom panel, left: display and quick launch

The display icon toggles the overview. The rocket in the middle is quick
launch, which offers the apps you actually start on the current workspace.

### Bottom panel, right: windows and tray

![The window list and the expanded tray](docs/bottom-bar.png)

The windows on the current workspace, then the tray, open here with every status
icon showing.

![The card for a window in the window list](docs/window-tooltip.png)

Hover a window for its title, app id, window id and how long ago it had focus;
the nix package it came from; the profile, binary, command and systemd scope it
runs under; and a live process tree with CPU, memory, threads and age per
process.

### Out of sight

- **Systemd health.** Failed units and kernel errors appear as a group of their
  own, which opens the journal for them. It only shows up when something is
  wrong.
- **Notifications.** The speech bubble next to the clock.

## Run

Inside the Nix devshell (`nix develop`):

```bash
cargo r
```

A debug build, running both panels.

Each output gets one transparent fullscreen layer surface that draws the top
bar, the bottom bar and the sidebar, with their shadows. It never resizes, so
groups expand without the compositor reallocating anything. Clicks outside
the bars pass through to the windows, and three small surfaces reserve the
edges so windows tile beside the panels.

## Install

Import `homeModules.default` from the flake. It installs the package, writes
`~/.config/vetrata.toml`, makes niri include `~/.config/vetrata/niri.kdl`
(the layer rules vetrata writes for its own surfaces) and, unless
`services.vetrata.systemdUnit` is disabled, a systemd user service bound to
`graphical-session.target`.

## Configure

`$XDG_CONFIG_HOME/vetrata.toml`, or `services.vetrata.settings`, which is
written to it verbatim. The keys mirror the structs in `src/config/` — unknown
ones are rejected rather than ignored.

`other_time_zones` lists IANA zones, `["Asia/Shanghai"]` by default, and the
clock group shows the time in each of those cities next to the local time and
date. An empty list hides them.

`[temperature.cpu]` and `[temperature.drive]` pick the sensor behind the CPU
and drive temperature readings and when they turn warning or critical. Without
them the CPU reading comes from `coretemp`, `k10temp` or `zenpower`, or else
from a CPU or SoC thermal zone as on ARM boards, and the drive reading from the
hwmon chip of the disk holding `/`, preferring its `Composite` sensor. The
thresholds follow the limits the hardware reports, and fixed defaults when it
reports none. `sensor` names a chip from `/sys/class/hwmon/*/name` and,
optionally, a `temp*_label`; `warning_celsius` and `critical_celsius` override
either threshold on its own:

```toml
[temperature.cpu]
sensor = { chip = "k10temp", label = "Tctl" }
critical_celsius = 90

[temperature.drive]
sensor = { chip = "drivetemp" }
warning_celsius = 55
```

The bar watches the file and applies every change in place, including a symlink
swapped to point elsewhere. An invalid file raises a critical toast with the
error and the bar keeps its previous configuration (defaults when it starts on
an invalid file). What you toggle from the settings panel — float offset, tint,
corner radius, shadow, grouping, workspace placement, do-not-disturb and toast
sound — is remembered separately and never read from this file. Blur and xray
from the niri group live only in `~/.config/vetrata/niri.kdl`.

## Secrets

Four optional files, all read from `$XDG_CONFIG_HOME/vetrata/`. No environment
variable, no configuration key, no fallback location. An absent file hides the
widget group that needs it.

| file | what it is |
| --- | --- |
| `openrouter-management-key` | An OpenRouter **management** key, plain text. A regular API key is not enough: the usage widget calls `/analytics/query`. |
| `claude-code-oauth` | The Claude Code credentials JSON (`~/.claude/.credentials.json`, containing `claudeAiOauth.accessToken`). A Pro or Max login shows the usage windows, each projected at its current pace to when it runs out or how full it is at reset, and, with `ccusage` installed, the active 5-hour block. An Enterprise login or no login shows spend per day, week and month from `ccusage`, projected over `working_days` (Monday to Friday by default). |
| `gh-auth-token` | A bare `gho_` GitHub OAuth token, used to call `/copilot_internal/user`. |
| `unsplash-access-key` | An Unsplash **access** key — the Client-ID of an app registered at <https://unsplash.com/developers>, never the secret key. |

`services.vetrata.secretFiles.<name>` symlinks each from wherever the secret
actually lives, so nothing lands in the nix store, and a `cargo r` build sees
the same files as the installed binary.
