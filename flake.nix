{
  description = "odin-rs — iced + iced_layershell bar for niri";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
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
      # on the *installed* binary (`nix build .#odin-rs` / `nix run`).
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

      odin-rs = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.makeWrapper ];
          # `--prefix` (prepend), not `--set`: compose with any values the
          # session already exports rather than clobbering them.
          postInstall = ''
            wrapProgram $out/bin/odin-rs \
              --prefix PATH : ${gsettingsRuntime.binPath} \
              --prefix XDG_DATA_DIRS : ${gsettingsRuntime.schemaDir} \
              --prefix GIO_EXTRA_MODULES : ${gsettingsRuntime.gioModules}
          '';
          meta.mainProgram = "odin-rs";
          meta.platforms = pkgs.lib.platforms.linux;
        }
      );
    in
    {
      packages.${system} = {
        default = odin-rs;
      };

      # `autoAddDriverRunpath` (above) patches RUNPATH on the installed binary
      # at build time, so `nix build .#odin-rs && ./result/bin/odin-rs`
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
        packages = commonArgs.nativeBuildInputs ++ commonArgs.buildInputs ++ [ pkgs.glib.bin ];
        env.LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath commonArgs.buildInputs;
        shellHook = ''
          export XDG_DATA_DIRS="${gsettingsRuntime.schemaDir}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
          export GIO_EXTRA_MODULES="${gsettingsRuntime.gioModules}''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
        '';
      };
    };
}
