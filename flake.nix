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

      simpleIcons = pkgs.fetchFromGitHub {
        owner = "simple-icons";
        repo = "simple-icons";
        rev = "16.35.0";
        hash = "sha256-RTQ3d3deMJQ8ToZVtoIT9jG47S32yp2ZDu/8BjMN5dg=";
      };

      commonArgs = {
        inherit src;
        version = (pkgs.lib.importTOML ./Cargo.toml).workspace.package.version;
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
        SIMPLE_ICONS_SVG = "${simpleIcons}/icons";
      };

      shellArgs = commonArgs // {
        pname = "sottomano";
        cargoExtraArgs = "--locked -p sottomano-shell";
      };

      showcaseAppIcons = "${pkgs.papirus-icon-theme}/share/icons/Papirus/64x64/apps";

      showcaseArgs = commonArgs // {
        SHOWCASE_APP_ICONS = showcaseAppIcons;
        pname = "sottomano-widget-showcase";
        cargoExtraArgs = "--locked -p sottomano-widget-showcase";
      };

      workspaceArgs = commonArgs // {
        SHOWCASE_APP_ICONS = showcaseAppIcons;
        pname = "sottomano-workspace";
        cargoExtraArgs = "--locked --workspace";
        CARGO_PROFILE = "dev";
        CARGO_PROFILE_DEV_DEBUG = "0";
      };

      formatters = [
        pkgs.treefmt
        pkgs.nixfmt
        (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
      ];

      shellArtifacts = craneLib.buildDepsOnly shellArgs;

      workspaceArtifacts = craneLib.buildDepsOnly workspaceArgs;

      showcaseArtifacts = craneLib.buildDepsOnly (showcaseArgs // showcaseFonts);

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

      allocatorEnv = {
        MALLOC_ARENA_MAX = "2";
      };

      allocatorWrapperFlags = pkgs.lib.concatStringsSep " " (
        pkgs.lib.mapAttrsToList (
          name: value: "--set-default ${name} ${pkgs.lib.escapeShellArg value}"
        ) allocatorEnv
      );

      sottomano = craneLib.buildPackage (
        shellArgs
        // {
          cargoArtifacts = shellArtifacts;
          nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.makeWrapper ];
          postInstall = ''
            wrapProgram $out/bin/sottomano \
              ${allocatorWrapperFlags} \
              --prefix PATH : ${gsettingsRuntime.binPath} \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps} \
              --prefix LD_LIBRARY_PATH : ${runtimeLibraryPath} \
              --prefix XDG_DATA_DIRS : ${gsettingsRuntime.schemaDir} \
              --prefix XDG_DATA_DIRS : ${soundThemeShare} \
              --prefix GIO_EXTRA_MODULES : ${gsettingsRuntime.gioModules}
          '';
          meta.mainProgram = "sottomano";
          meta.platforms = pkgs.lib.platforms.linux;
          meta.license = pkgs.lib.licenses.gpl3Plus;
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
          preBuild = ''
            export HOME=$TMPDIR
            export XDG_CACHE_HOME=$TMPDIR/cache
          '';
          buildPhaseCargoCommand = "cargoWithProfile run --quiet $cargoExtraArgs -- --output $out";
          installPhaseCommand = "";
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
        format =
          pkgs.runCommand "sottomano-format"
            {
              nativeBuildInputs = formatters;
            }
            ''
              export HOME=$TMPDIR
              cp -r ${src} source
              chmod -R u+w source
              cd source
              treefmt --ci --tree-root .
              touch $out
            '';
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
          commonArgs.nativeBuildInputs
          ++ commonArgs.buildInputs
          ++ [ pkgs.glib.bin ]
          ++ formatters
          ++ runtimeDeps;
        env = allocatorEnv // {
          LD_LIBRARY_PATH = runtimeLibraryPath;
          WEATHER_ICONS_SVG = "${pkgs.weather-icons.src}/svg";
          PHOSPHOR_SVG = "${phosphorCore}/assets";
          SIMPLE_ICONS_SVG = "${simpleIcons}/icons";
          SHOWCASE_APP_ICONS = showcaseAppIcons;
        };
        shellHook = ''
          export XDG_DATA_DIRS="${gsettingsRuntime.schemaDir}:${soundThemeShare}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
          export GIO_EXTRA_MODULES="${gsettingsRuntime.gioModules}''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
        '';
      };
    };
}
