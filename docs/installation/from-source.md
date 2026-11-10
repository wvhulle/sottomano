# From source

Sottomano builds with cargo on a nightly toolchain, which rustup picks up from
`rust-toolchain.toml` in the checkout:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

The build headers it needs come from the distribution's package manager:
[Ubuntu](ubuntu.md#build-headers) or [Fedora](fedora.md#build-headers).

The icon crates embed SVGs from three icon sets at build time, so check them
out at the revisions the flake pins and point the build at them:

```bash
mkdir -p ~/src && cd ~/src
git clone https://github.com/phosphor-icons/core phosphor-core
git -C phosphor-core checkout 2b75f3ad12b420c9504ef05df8d2564a28f8500e
git clone --depth 1 --branch 16.35.0 https://github.com/simple-icons/simple-icons
git clone --depth 1 --branch 2.0.12 https://github.com/erikflowers/weather-icons

git clone https://codeberg.org/wvhulle/sottomano && cd sottomano
export PHOSPHOR_SVG=~/src/phosphor-core/assets
export SIMPLE_ICONS_SVG=~/src/simple-icons/icons
export WEATHER_ICONS_SVG=~/src/weather-icons/svg
cargo install --locked --path sottomano-shell
```

To compile in only your compositor, add
`--no-default-features --features niri,libvirt-domains` (or `sway`, `hyprland`)
to `cargo install`.

The widgets call distribution tools and D-Bus services when present and hide
what they cannot read; the lists are on each
[distribution page](ubuntu.md#runtime-tools-and-services). The system pieces
some widgets read need a manual [system setup](system-setup.md).