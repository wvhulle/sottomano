# Vetrata

My personal desktop shell for the [niri](https://github.com/YaLTeR/niri)
compositor: two panels in one binary, built on [iced](https://iced.rs) and
[iced_layershell](https://crates.io/crates/iced_layershell), reading D-Bus with
`zbus` where a service exists and calling the CLI where one does not.

The shell explains itself: hover any reading for a card with what it shows,
and the card's top line says what a click does.

![The top panel, groups expanded, the CPU card open](docs/top-bar.png)
![The bottom panel: window list and tray](docs/bottom-bar.png)

## Run

Inside the Nix devshell (`nix develop`):

```bash
cargo r
```

`vetrata --help` lists the options that describe its setup instead of starting
the shell.

## Install

Import `homeModules.default` from the flake. It installs the package, writes
`~/.config/vetrata.toml`, makes niri include `~/.config/vetrata/niri.kdl`
(the layer rules vetrata writes for its own surfaces) and, unless
`services.vetrata.systemdUnit` is disabled, a systemd user service bound to
`graphical-session.target`.

Several widgets read system daemons a home-manager module cannot start. On
NixOS, import `nixosModules.default` and set `programs.vetrata.enable = true`.
It also unlocks root-only CPU readings and the X3D preference switch; turn
`programs.vetrata.cpuPowerReading.enable` off on machines shared with untrusted
users.

## Configure

`$XDG_CONFIG_HOME/vetrata.toml`, or `services.vetrata.settings`, which is
written to it verbatim. `vetrata --print-default-config` prints every key with
its default. Unknown keys are rejected. The bar applies every change to the
file in place; an invalid file raises a toast and keeps the previous
configuration.

Temperature sensors are detected automatically, so their keys don't appear in
the printed defaults. To pin a sensor or override a threshold:

```toml
[temperature.cpu]
sensor = { chip = "k10temp", label = "Tctl" }
critical_celsius = 90
```

AI usage readings raise a toast when the current pace would use up a window
or monthly limit before it resets. Spend estimated from local logs has no
limit of its own; give it one in dollars to be warned the same way:

```toml
[widget_groups.ai_usage.monthly_budget_usd]
claude_code = 100
copilot_cli = 20
```

## Secrets

`vetrata --list-secrets` lists the optional secret files, what each enables and
whether it is present. `services.vetrata.secretFiles.<name>` symlinks each from
wherever the secret actually lives, so nothing lands in the nix store.
