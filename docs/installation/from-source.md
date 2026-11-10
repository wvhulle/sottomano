# From source

Sottomano builds with cargo on a nightly toolchain, which
[rustup](https://rustup.rs) picks up from `rust-toolchain.toml` in the
checkout. The build headers it needs come from the distribution's package
manager: [Ubuntu](ubuntu.md#build-headers) or
[Fedora](fedora.md#build-headers).

```bash
git clone https://codeberg.org/wvhulle/sottomano && cd sottomano
eval "$(cargo xtask fetch-icon-svgs)"
cargo install --locked --path sottomano-shell
```

`cargo xtask fetch-icon-svgs` clones the three icon sets the icon crates embed
SVGs from — phosphor, simple-icons and weather-icons — into
`~/.cache/sottomano/icon-sets/`, each at the revision the flake pins, and
prints the `PHOSPHOR_SVG`, `SIMPLE_ICONS_SVG` and `WEATHER_ICONS_SVG` exports
the build reads (a Nix build does this itself). The phosphor and simple-icons
revisions come from `flake.nix`; the weather-icons version comes from the
nixpkgs the flake pins, so that one needs `nix eval --inputs-from .`.

To compile in only your compositor, add
`--no-default-features --features niri,libvirt-domains` (or `sway`, `hyprland`)
to `cargo install`.

The widgets call distribution tools and D-Bus services when present and hide
what they cannot read; `sudo sottomano setup <distro>` installs them, as the
[distribution pages](ubuntu.md) show. The system pieces some widgets read need
the same command's [system setup steps](system-setup.md).