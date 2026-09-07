# Vetrata

My personal desktop shell existing out of two panels.

Current focused on the [niri](https://github.com/YaLTeR/niri)
compositor exclusively.

One compiled binary built on:

- [iced](https://iced.rs). My first project with this framework.
- [iced_layershell](https://crates.io/crates/iced_layershell):
- D-Bus calls with `zbus` as much as possible.
- CLI calls otherwise

## Run

Assuming you are running a shell inside the Nix devshell:

```bash
cargo r
```

This compiles a quick debug build and runs the two panels.

## Secrets

The AI widgets read three optional token files, all from
`$XDG_CONFIG_HOME/vetrata/`. A file that is absent hides its pills. There is no
environment variable, no configuration key and no fallback location — put the
file there, or symlink it there.

`openrouter-management-key` — an OpenRouter **management** key as plain text. A
regular API key is not enough: the usage widget calls `/analytics/query`, which
only management keys may access.

`claude-code-oauth` — the Claude Code OAuth credentials JSON (the
`~/.claude/.credentials.json` blob containing `claudeAiOauth.accessToken`).
Drives the Claude subscription gauge; on API-key machines with no OAuth token it
surfaces `ccusage` daily spend instead.

`gh-auth-token` — a bare `gho_` GitHub OAuth token. The Copilot usage pills read
it and call `/copilot_internal/user` directly. Without it the Copilot quota
pills are hidden; the local-`ccusage` Copilot pill needs no token and still
shows if `ccusage` is installed.

`unsplash-access-key` — an Unsplash **access key**, the Client-ID of an
application registered at <https://unsplash.com/developers>. Drives the
wallpaper pill described below. The Unsplash *secret* key is only used by the
OAuth flow and is never sent, so do not put it here.

Because the location is fixed, a `cargo r` build sees the same secrets as the
installed one. With the home-manager module, set
`services.vetrata.secretFiles.<name>` to wherever the secret actually lives
(an agenix path, another tool's credentials file) and it is symlinked into
place rather than copied, so nothing lands in the nix store.

## Wallpaper

The wallpaper pill names the place the current wallpaper was photographed in and
summarises the Wikipedia article for that place. It asks `swww query` (or the
`awww` fork, whichever is installed) which image
is on screen every 30 s; when the image changes it reads the Unsplash photo id
out of the filename — Unsplash downloads are named
`firstname-lastname-<id>-unsplash.jpg` — and resolves that id once against the
Unsplash and Wikipedia APIs.

No image is ever uploaded or inspected, only its filename, so wallpapers that did
not come from Unsplash resolve to nothing and the pill hides itself. Unsplash
fills in a location for roughly half its photos; without one the tooltip still
carries the caption and photographer.

There is nothing to configure. The pill appears when both halves are actually
there — `awww` or `swww` on the session PATH (the same way `hyprlock` and the
terminal are) and `unsplash-access-key` in place — and hides otherwise, like
every other secret-backed pill. The Wikipedia edition follows the session
locale, taking the language subtag of `LC_ALL`, `LC_MESSAGES` or `LANG` and
falling back to `en`, so a `nl_BE.UTF-8` session reads nl.wikipedia.org.

The daemons are tried in that order, and any other swww fork works too as long
as its `query` prints `currently displaying: image: <path>`. Neither binary is
bundled: shipping one would shadow yours on PATH, and a client/daemon version
mismatch across that socket would break wallpaper switching.

## History

This is a Rust rewrite of my other project [odin](https://codeberg.org/wvhulle/odin).
