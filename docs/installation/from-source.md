# From source

Sottomano builds with cargo on a nightly toolchain, which
[rustup](https://rustup.rs) picks up from `rust-toolchain.toml` in the
checkout. The build headers it needs come from the distribution's package
manager: [Ubuntu](ubuntu.md#build-headers) or
[Fedora](fedora.md#build-headers).

```bash
git clone https://codeberg.org/wvhulle/sottomano && cd sottomano
eval "$(nix run .#icon-svg-exports)"
cargo install --locked --path sottomano-shell
```

The icon crates embed SVGs from three icon sets — phosphor, simple-icons and
weather-icons — and read their directories from `PHOSPHOR_SVG`,
`SIMPLE_ICONS_SVG` and `WEATHER_ICONS_SVG`. `nix run .#icon-svg-exports`
fetches the sets at the revisions the flake pins and prints those exports, so
this step needs [Nix](nix.md#on-other-distributions) with flakes enabled; a Nix build
and `nix develop` set the variables themselves.

To compile in only your compositor, add
`--no-default-features --features niri,libvirt-domains` (or `sway`, `hyprland`)
to `cargo install`.

The widgets call distribution tools and D-Bus services when present and hide
what they cannot read; `sudo sottomano setup <distro>` installs them, as the
[distribution pages](ubuntu.md) show. The system pieces some widgets read need
the same command's [system setup steps](system-setup.md).