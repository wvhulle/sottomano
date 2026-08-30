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

Because the location is fixed, a `cargo r` build sees the same secrets as the
installed one. With the home-manager module, set
`services.vetrata.secretFiles.<name>` to wherever the secret actually lives
(an agenix path, another tool's credentials file) and it is symlinked into
place rather than copied, so nothing lands in the nix store.

## History

This is a Rust rewrite of my other project [odin](https://codeberg.org/wvhulle/odin).
