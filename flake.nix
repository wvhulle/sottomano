{
  description = "sottomano — iced + iced_exwlshell shell for niri";

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

      src = ./.;

      phosphorCore = pkgs.fetchFromGitHub {
        owner = "phosphor-icons";
        repo = "core";
        rev = "2b75f3ad12b420c9504ef05df8d2564a28f8500e";
        hash = "sha256-aasAz4rTllySAxSvYORu+jIKqbqPIFCDM6s8pkNiW2o=";
      };

      commonArgs = {
        inherit src;
        version = "0.1.0";
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
        WEATHER_ICONS_SVG = "${pkgs.weather-icons.src}/svg";
        PHOSPHOR_SVG = "${phosphorCore}/assets";
      };

      shellArgs = commonArgs // {
        pname = "sottomano";
        cargoExtraArgs = "--locked -p sottomano-shell";
      };

      showcaseArgs = commonArgs // {
        pname = "sottomano-widget-showcase";
        cargoExtraArgs = "--locked -p sottomano-widget-showcase";
      };

      workspaceArgs = commonArgs // {
        pname = "sottomano-workspace";
        cargoExtraArgs = "--locked --workspace";
      };

      shellArtifacts = craneLib.buildDepsOnly shellArgs;

      workspaceArtifacts = craneLib.buildDepsOnly workspaceArgs;

      showcaseArtifacts = craneLib.buildDepsOnly showcaseArgs;

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
          inxi
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

      runtimeLibraryPath = "${pkgs.lib.makeLibraryPath commonArgs.buildInputs}:${pkgs.addDriverRunpath.driverLink}/lib";

      sottomano = craneLib.buildPackage (
        shellArgs
        // {
          cargoArtifacts = shellArtifacts;
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.makeWrapper ];
          postInstall = ''
            wrapProgram $out/bin/sottomano \
              --prefix PATH : ${gsettingsRuntime.binPath} \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps} \
              --prefix LD_LIBRARY_PATH : ${runtimeLibraryPath} \
              --prefix XDG_DATA_DIRS : ${gsettingsRuntime.schemaDir} \
              --prefix XDG_DATA_DIRS : ${soundThemeShare} \
              --prefix GIO_EXTRA_MODULES : ${gsettingsRuntime.gioModules}
          '';
          meta.mainProgram = "sottomano";
          meta.platforms = pkgs.lib.platforms.linux;
        }
      );

      showcaseFonts = {
        nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.fontconfig ];
        FONTCONFIG_FILE = pkgs.makeFontsConf {
          fontDirectories = [
            pkgs.ibm-plex
            pkgs.nerd-fonts.lilex
            pkgs.dejavu_fonts
          ];
          includes = [ ./nix/fonts.conf.d ];
        };
      };

      readmeRecordings = craneLib.mkCargoDerivation (
        showcaseArgs
        // showcaseFonts
        // {
          cargoArtifacts = showcaseArtifacts;
          LANG = "en_US.UTF-8";
          VK_ICD_FILENAMES = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.vulkan-loader ];
          doInstallCargoArtifacts = false;
          buildPhaseCargoCommand = ''
            mkdir -p recordings
            cargoWithProfile run $cargoExtraArgs
          '';
          installPhaseCommand = ''
            mkdir -p $out
            cp recordings/*.webp $out/
          '';
        }
      );

      homeModule = import ./nix/hm-module.nix { inherit self; };
    in
    {
      packages.${system} = {
        default = sottomano;
        sottomano = sottomano;
        deps = showcaseArtifacts;
        readme-recordings = readmeRecordings;
      };

      checks.${system} = {
        tests = craneLib.cargoTest (
          workspaceArgs
          // showcaseFonts
          // {
            cargoArtifacts = workspaceArtifacts;
            doInstallCargoArtifacts = false;
            RUST_BACKTRACE = "1";
          }
        );
        clippy = craneLib.cargoClippy (
          workspaceArgs
          // {
            cargoArtifacts = workspaceArtifacts;
            cargoClippyExtraArgs = "--all-targets";
          }
        );
        fmt = craneLib.cargoFmt { inherit src; };
      };

      homeModules.default = {
        imports = [
          niri-hm.homeModules.default
          homeModule
        ];
      };

      nixosModules.default = ./nix/nixos-module.nix;

      devShells.${system}.default = craneLib.devShell {
        packages =
          commonArgs.nativeBuildInputs ++ commonArgs.buildInputs ++ [ pkgs.glib.bin ] ++ runtimeDeps;
        env.LD_LIBRARY_PATH = runtimeLibraryPath;
        env.WEATHER_ICONS_SVG = "${pkgs.weather-icons.src}/svg";
        env.PHOSPHOR_SVG = "${phosphorCore}/assets";
        shellHook = ''
          export XDG_DATA_DIRS="${gsettingsRuntime.schemaDir}:${soundThemeShare}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
          export GIO_EXTRA_MODULES="${gsettingsRuntime.gioModules}''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
        '';
      };
    };
}
