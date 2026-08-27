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

The AI widgets read three optional token files. Each is looked up by trying
the paths below in order and taking the first one that exists; when none do,
the corresponding pills stay hidden. There is no environment variable and no
configuration option — deploy the file to one of these paths.

**OpenRouter management key** — plain text. A regular API key is not enough:
the usage widget calls `/analytics/query`, which only management keys may
access.

1. `$XDG_CONFIG_HOME/vetrata/openrouter-management-key`
2. `/run/agenix/openrouter-management-key`

**Claude Code OAuth credentials** — the `~/.claude/.credentials.json` blob
containing `claudeAiOauth.accessToken`. Drives the Claude subscription gauge;
on API-key machines (no OAuth token) it surfaces `ccusage` daily spend instead.

1. `~/.claude/.credentials.json`
2. `/run/agenix/claude-code-oauth`

**GitHub OAuth token** — a bare `gho_` token. The Copilot usage pills read it
and call `/copilot_internal/user` directly. Without it the Copilot quota pills
are hidden; the local-`ccusage` Copilot pill needs no token and still shows if
`ccusage` is installed.

1. `$XDG_STATE_HOME/chatbot/copilot-token`
2. `/run/agenix/gh-auth-token`

The `/run/agenix` entries are the agenix defaults, so a `cargo r` build picks
up secrets already deployed on the machine without going through the systemd
unit.

## History

This is a Rust rewrite of my other project [odin](https://codeberg.org/wvhulle/odin).
