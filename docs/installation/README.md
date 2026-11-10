# Installation

Sottomano runs on [niri](../compositors/niri.md), [sway](../compositors/sway.md)
and [Hyprland](../compositors/hyprland.md), and finds the running one from
`NIRI_SOCKET`, `HYPRLAND_INSTANCE_SIGNATURE` or `SWAYSOCK`. Each compositor's
page covers starting the session and including the window styling rules
sottomano writes, and lists what that compositor does not offer. For installing
the compositor itself, follow the links on its page.

The pages under this one are organized by package manager:

- [Nix](nix.md): the flake's package, the home-manager module for the bar, the
  NixOS module for the system pieces some widgets read, and the `nix profile`
  route for other distributions.
- [From source](from-source.md): building with cargo, for which each
  distribution's page lists its build headers.
- [Ubuntu](ubuntu.md) and [Fedora](fedora.md): each distribution's build
  headers and the `sudo sottomano setup <distro>` command that sets up the rest.
- [System setup without NixOS](system-setup.md): the distro-neutral equivalent
  of the NixOS module's system pieces, `sudo sottomano setup linux` and what
  each step does.
- [Session startup](../session.md): the systemd user service that starts the
  bar with the graphical session, outside home-manager.

## Try it from a checkout

On any distribution with Nix and flakes enabled:

```bash
git clone https://codeberg.org/wvhulle/sottomano
cd sottomano
nix develop
cargo r
```

Or run the packaged binary without cloning:

```bash
nix run git+https://codeberg.org/wvhulle/sottomano
```

`sottomano --help` lists the subcommands that describe the setup without
starting the shell, such as `default-config` and `secrets`.