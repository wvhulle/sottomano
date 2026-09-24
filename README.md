# Vetrata

My personal desktop shell for the [niri](https://github.com/YaLTeR/niri)
compositor: two panels in one binary, built on [iced](https://iced.rs) and
[iced_layershell](https://crates.io/crates/iced_layershell), reading D-Bus with
`zbus` where a service exists and calling the CLI where one does not.

![The top panel](docs/top-bar.png)

![The bottom panel](docs/bottom-bar.png)

## What's different

### What it shows you

![The AI, idle, media and volume widget groups](docs/indicators.png)

- **AI harness usage.** Claude subscription, Copilot quota and OpenRouter spend,
  tracked while you work.
- **Media that plays along.** The current MPRIS track, with play/pause and a
  jump back to the app that owns it.
- **Systemd health.** Failed units and kernel errors become a widget group that
  opens the journal for them.
- **Where your wallpaper was taken.** The current wallpaper's location, with a
  summary of its Wikipedia article.

### What hovering reveals

![The tooltip for a window in the window list](docs/window-tooltip.png)

- **Cards that grow and morph.** A tooltip expands out of the widget group you
  hovered, and sliding along the bar morphs that card into the next rather than
  popping a new one.
- **Windows explain themselves.** Hover an entry in the window list for its app
  id, focus age, the command behind it and the nix packages it came from, with a
  live process tree underneath.
- **Copyable values.** Click an address, a PID, a network name in a tooltip and
  it goes to the clipboard.

### What you can do from the bar

![The workspace widget group, focus marker over the current workspace](docs/workspaces.png)

- **The focus marker slides.** Switch workspace or window and the indicator
  glides to its new place instead of blinking there.
- **Settings adjust in place.** Idle timers, backlight, volume, colour
  temperature and the panel's own shape are sliders in the bar itself.
- **Quick launch knows your workspace.** Each workspace offers the apps you
  actually start there.

## Run

Inside the Nix devshell (`nix develop`):

```bash
cargo r
```

A debug build, running both panels.

## Install

Import `homeModules.default` from the flake. It installs the package, writes
`~/.config/vetrata.toml`, adds the niri layer rules and, unless
`services.vetrata.systemdUnit` is disabled, a systemd user service bound to
`graphical-session.target`.

## Configure

`~/.config/vetrata.toml`, or `services.vetrata.settings`, which is written to it
verbatim. The keys mirror the structs in `src/config/` — unknown ones are
rejected rather than ignored.

## Secrets

Four optional files, all read from `$XDG_CONFIG_HOME/vetrata/`. No environment
variable, no configuration key, no fallback location. An absent file hides the
widget group that needs it.

| file | what it is |
| --- | --- |
| `openrouter-management-key` | An OpenRouter **management** key, plain text. A regular API key is not enough: the usage widget calls `/analytics/query`. |
| `claude-code-oauth` | The Claude Code credentials JSON (`~/.claude/.credentials.json`, containing `claudeAiOauth.accessToken`). Without an OAuth token it falls back to `ccusage` daily spend. |
| `gh-auth-token` | A bare `gho_` GitHub OAuth token, used to call `/copilot_internal/user`. |
| `unsplash-access-key` | An Unsplash **access** key — the Client-ID of an app registered at <https://unsplash.com/developers>, never the secret key. |

`services.vetrata.secretFiles.<name>` symlinks each from wherever the secret
actually lives, so nothing lands in the nix store, and a `cargo r` build sees
the same files as the installed binary.
