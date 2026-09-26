{
  description = "vetrata — iced + iced_layershell bar for niri";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    niri-hm = {
      url = "git+https://codeberg.org/wvhulle/niri-hm?ref=main";
      inputs.nixpkgs.follows = "nixpkgs";
    };
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
        p: p.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml
      );

      src = craneLib.cleanCargoSource ./.;

      commonArgs = {
        inherit src;
        strictDeps = true;
        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.autoAddDriverRunpath
          pkgs.rustPlatform.bindgenHook
        ];
        buildInputs = with pkgs; [
          wayland
          libxkbcommon
          libGL
          vulkan-loader
          systemd
          pipewire
        ];
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      gsettingsRuntime = {
        binPath = "${pkgs.glib.bin}/bin";
        schemaDir = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}";
        gioModules = "${pkgs.dconf.lib}/lib/gio/modules";
      };

      soundThemeShare = "${pkgs.sound-theme-freedesktop}/share";

      runtimeDeps =
        (with pkgs; [
          iproute2
          networkmanager
          modemmanager
          bluez
          wireplumber
          pipewire
          wireguard-tools
          tailscale
          xdg-utils
          wl-clipboard
        ])
        ++ [
          umbra.packages.${system}.default
        ];

      vetrata = craneLib.buildPackage (
        commonArgs
        // {
          inherit cargoArtifacts;
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.makeWrapper ];
          # Mesa lists Rgb10a2Unorm (2-bit alpha) first and iced 0.14 picks it, turning
          # translucent shadows into flat bands. Drop once iced skips it (upstream ed36c6e107).
          postInstall = ''
            wrapProgram $out/bin/vetrata \
              --prefix PATH : ${gsettingsRuntime.binPath} \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps} \
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath commonArgs.buildInputs} \
              --prefix XDG_DATA_DIRS : ${gsettingsRuntime.schemaDir} \
              --prefix XDG_DATA_DIRS : ${soundThemeShare} \
              --prefix GIO_EXTRA_MODULES : ${gsettingsRuntime.gioModules} \
              --set vk_wsi_force_bgra8_unorm_first true
          '';
          meta.mainProgram = "vetrata";
          meta.platforms = pkgs.lib.platforms.linux;
        }
      );

      homeModule = import ./hm-module.nix { inherit self; };
    in
    {
      packages.${system} = {
        default = vetrata;
        vetrata = vetrata;
        deps = cargoArtifacts;
      };

      homeModules.default = {
        imports = [
          niri-hm.homeModules.default
          homeModule
        ];
      };

      devShells.${system}.default = craneLib.devShell {
        packages =
          commonArgs.nativeBuildInputs ++ commonArgs.buildInputs ++ [ pkgs.glib.bin ] ++ runtimeDeps;
        env.LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath commonArgs.buildInputs;
        env.vk_wsi_force_bgra8_unorm_first = "true";
        shellHook = ''
          export XDG_DATA_DIRS="${gsettingsRuntime.schemaDir}:${soundThemeShare}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
          export GIO_EXTRA_MODULES="${gsettingsRuntime.gioModules}''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
        '';
      };
    };
}
