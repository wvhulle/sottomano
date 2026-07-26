{
  description = "vetrata — iced + iced_layershell bar for niri";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # Theme-blind niri module. The home-manager module contributes
    # services.niri.settings.layer-rules (the gradient-shadow casters), so it
    # imports this to keep that option in scope and validated. niri-hm exports
    # its module as a path, so a desktop that also imports it deduplicates to one
    # instance.
    niri-hm = {
      url = "git+https://codeberg.org/wvhulle/niri-hm";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # umbra: the dark/light-mode CLI the brightness group shells out to
    # (`umbra get` / `umbra toggle`). Only the binary is needed — vetrata does
    # not register with the services.umbra theme registry — so it goes on PATH
    # via runtimeDeps, like situo's CLI in odin. clight / clightd / wl-gammarelay
    # are D-Bus daemons read over their session/system interfaces (the user's own
    # config), so they are not bundled here.
    umbra = {
      url = "git+https://codeberg.org/wvhulle/umbra";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
      niri-hm,
      umbra,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain (
        pkgs.rust-bin.nightly.latest.default.override {
          extensions = [
            "rust-src"
            "rustfmt"
            "clippy"
            "rust-analyzer"
          ];
        }
      );

      src = craneLib.cleanCargoSource ./.;

      # System libs iced_layershell + wgpu / winit dlopen at run time.
      # `autoAddDriverRunpath` is a setup hook: it patches the built binary's
      # RUNPATH to include every library in `buildInputs`, so dlopen'd libs
      # (vulkan-loader, libGL) resolve at run time without any LD_LIBRARY_PATH
      # on the *installed* binary (`nix build .#vetrata` / `nix run`).
      commonArgs = {
        inherit src;
        strictDeps = true;
        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.autoAddDriverRunpath
        ];
        buildInputs = with pkgs; [
          wayland
          libxkbcommon
          libGL
          vulkan-loader
          # libsystemd, for the `systemd` crate's journal reader (kernel-errors
          # widget). Linked, not dlopen'd, so no RUNPATH/runtimeDeps entry.
          systemd
        ];
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      # `freedesktop_icons::default_theme_gtk()` (used to pin the icon-theme for
      # window-list app icons) picks the theme by shelling out to
      # `gsettings get org.gnome.desktop.interface icon-theme`. That needs the
      # `gsettings` binary on PATH, the `org.gnome.desktop.interface` schema on
      # XDG_DATA_DIRS, and the dconf GIO backend on GIO_EXTRA_MODULES (without
      # dconf, gsettings returns the schema default instead of the value the
      # session persisted). Missing any of the three makes the call return None,
      # leaving only hicolor-backed icons (Firefox, kitty) visible. Shared by
      # the wrapper and the devShell so the two can't drift.
      gsettingsRuntime = {
        binPath = "${pkgs.glib.bin}/bin";
        schemaDir = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}";
        gioModules = "${pkgs.dconf.lib}/lib/gio/modules";
      };

      # Standard system CLIs the bar's data sources shell out to at run time.
      # Put on the wrapped binary's PATH and into the dev shell, so both
      # `nix run` and `nix develop -c cargo run` resolve them regardless of the
      # ambient session. The wvhulle *daemons* are not here — situo, clight,
      # clightd and wl-gammarelay are read over their D-Bus interfaces and the
      # idle daemon runs in-process, so their binaries aren't needed (their
      # daemons are the user's own config). umbra is the exception: the dark-mode
      # widget shells out to its CLI, so its binary is bundled (below).
      runtimeDeps = with pkgs; [
        iproute2 # ip route/addr — throughput, wireguard
        networkmanager # nmcli — ethernet, wifi, connectivity, vpn
        modemmanager # mmcli — mobile broadband
        bluez # bluetoothctl — bluetooth
        wireplumber # wpctl — audio volume
        wireguard-tools # wg — wireguard peer state
        tailscale # tailscale status
        xdg-utils # xdg-open — tailscale admin console
      ];

      vetrata = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.makeWrapper ];
          # `--prefix` (prepend), not `--set`: compose with any values the
          # session already exports rather than clobbering them.
          postInstall = ''
            wrapProgram $out/bin/vetrata \
              --prefix PATH : ${gsettingsRuntime.binPath} \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps} \
              --prefix XDG_DATA_DIRS : ${gsettingsRuntime.schemaDir} \
              --prefix GIO_EXTRA_MODULES : ${gsettingsRuntime.gioModules}
          '';
          meta.mainProgram = "vetrata";
          meta.platforms = pkgs.lib.platforms.linux;
        }
      );

      # Home-manager module lives in ./hm-module.nix to keep this file focused on
      # packaging. It needs the flake-level `self` (for the default package).
      homeModule = import ./hm-module.nix { inherit self; };
    in
    {
      packages.${system} = {
        default = vetrata;
      };

      # niri-hm provides the services.niri option surface the module's
      # layer-rules contribution writes to; its path-exported module dedupes when
      # a desktop also imports it directly.
      homeModules.default = {
        imports = [
          niri-hm.homeModules.default
          homeModule
        ];
      };

      # `autoAddDriverRunpath` (above) patches RUNPATH on the installed binary
      # at build time, so `nix build .#vetrata && ./result/bin/vetrata`
      # resolves dlopen'd libs. For `nix develop -c cargo run`, the dev-built
      # binary has no such RUNPATH, and `ash` (wgpu) + `wayland-egl` always
      # dlopen libvulkan / libwayland-egl with no static-link option. Declare
      # the loader path here so the dev shell mirrors what the package's
      # RUNPATH provides.
      #
      # The dev-built binary is also unwrapped, so mirror the wrapper's
      # gsettings runtime (see `gsettingsRuntime` above) here too — otherwise
      # `nix develop -c cargo run` resolves no icon theme. `pkgs.glib.bin` puts
      # `gsettings` on PATH; the shellHook prepends the schema and dconf paths,
      # composing with any session values.
      devShells.${system}.default = craneLib.devShell {
        packages =
          commonArgs.nativeBuildInputs ++ commonArgs.buildInputs ++ [ pkgs.glib.bin ] ++ runtimeDeps;
        env.LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath commonArgs.buildInputs;
        shellHook = ''
          export XDG_DATA_DIRS="${gsettingsRuntime.schemaDir}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
          export GIO_EXTRA_MODULES="${gsettingsRuntime.gioModules}''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
        '';
      };
    };
}
