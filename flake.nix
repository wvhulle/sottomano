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

      odin-rs = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;
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
      devShells.${system}.default = craneLib.devShell {
        packages = commonArgs.nativeBuildInputs ++ commonArgs.buildInputs;
        env.LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath commonArgs.buildInputs;
      };
    };
}
