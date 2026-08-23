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


## History

This is a Rust rewrite of my other project [odin](https://codeberg.org/wvhulle/odin).
