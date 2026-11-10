{
  description = "sottomano — iced + iced_exwlshell shell for niri, sway and Hyprland";

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

      vendoredCargoDeps = craneLib.vendorCargoDeps {
        inherit src;
        overrideVendorGitCheckout =
          checkoutPackages: checkoutDrv:
          if builtins.any (checkoutPackage: checkoutPackage.name == "niri-ipc") checkoutPackages then
            checkoutDrv.overrideAttrs (previousAttrs: {
              postPatch = (previousAttrs.postPatch or "") + ''
                rm -rf docs/wiki
              '';
            })
          else
            checkoutDrv;
      };

      commonArgs = {
        inherit src;
        cargoVendorDir = vendoredCargoDeps;
        version = (pkgs.lib.importTOML ./Cargo.toml).workspace.package.version;
        strictDeps = true;
        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.autoAddDriverRunpath
          pkgs.rustPlatform.bindgenHook
        ];
        buildInputs =
          runtimeDlopenLibraries
          ++ (with pkgs; [
            systemd
            pipewire
            openssl
            libvirt
          ]);
        WEATHER_ICONS_SVG = "${pkgs.weather-icons.src}/svg";
        PHOSPHOR_SVG = "${phosphorCore}/assets";
        SIMPLE_ICONS_SVG = "${simpleIcons}/icons";
      };

      shellArgs = commonArgs // {
        pname = "sottomano";
        cargoExtraArgs = "--locked -p sottomano-shell";
      };

      showcaseAppIcons = "${pkgs.papirus-icon-theme}/share/icons/Papirus/64x64/apps";

      sourcesAffectingRecordings =
        let
          inherit (pkgs.lib) fileset;
          showcaseCrateSources = fileset.toSource {
            root = ./.;
            fileset = fileset.difference (fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./.cargo
              ./corredo
              ./iced_controller_diagram
              ./sottomano-toml-config
              ./sottomano-phosphor-icons
              ./sottomano-simple-icons
              ./sottomano-weather-icons
              ./sottomano-desktop-sources
              ./sottomano-desktop-sinks
              ./sottomano-widgets
              ./sottomano-widget-showcase
              ./sottomano-shell/Cargo.toml
              ./xtask/Cargo.toml
            ]) ./sottomano-widget-showcase/tests;
          };
        in
        pkgs.runCommand "sottomano-showcase-source" { } ''
          cp -r ${showcaseCrateSources} $out
          chmod -R u+w $out
          mkdir -p $out/sottomano-shell/src $out/xtask/src
          touch $out/sottomano-shell/src/main.rs \
            $out/sottomano-shell/src/secret_memory_holders_main.rs \
            $out/xtask/src/main.rs
        '';

      showcaseArgs = commonArgs // {
        src = sourcesAffectingRecordings;
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
          fontconfig
          iproute2
          networkmanager
          modemmanager
          bluez
          inxi
          wireplumber
          pipewire
          openssl
          wireguard-tools
          tailscale
          xdg-utils
          wl-clipboard
          systemd
        ])
        ++ [
          umbra.packages.${system}.default
        ];

      runtimeDlopenLibraries = with pkgs; [
        wayland
        libxkbcommon
        libGL
        vulkan-loader
      ];

      devShellLibraryPath = "${pkgs.lib.makeLibraryPath (pkgs.lib.remove pkgs.openssl commonArgs.buildInputs)}:${pkgs.addDriverRunpath.driverLink}/lib";

      allocatorEnv = {
        MALLOC_ARENA_MAX = "2";
        GLIBC_TUNABLES = "glibc.malloc.mmap_threshold=131072:glibc.malloc.trim_threshold=131072";
      };

      allocatorWrapperFlags = pkgs.lib.concatStringsSep " " (
        pkgs.lib.mapAttrsToList (
          name: value: "--set-default ${name} ${pkgs.lib.escapeShellArg value}"
        ) allocatorEnv
      );

      allCompositors = [
        "niri"
        "sway"
        "hyprland"
      ];

      compositorCargoArgs =
        compositors:
        if pkgs.lib.naturalSort compositors == pkgs.lib.naturalSort allCompositors then
          shellArgs.cargoExtraArgs
        else
          "${shellArgs.cargoExtraArgs} --no-default-features --features ${
            pkgs.lib.concatStringsSep "," (compositors ++ [ "libvirt-domains" ])
          }";

      mkSottomano =
        {
          compositors ? allCompositors,
        }:
        craneLib.buildPackage (
          shellArgs
          // {
            cargoArtifacts = shellArtifacts;
            cargoExtraArgs = compositorCargoArgs compositors;
            nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.makeBinaryWrapper ];
            postInstall = ''
              mkdir -p $out/libexec
              mv $out/bin/sottomano $out/libexec/sottomano
              makeWrapper $out/libexec/sottomano $out/bin/sottomano \
                ${allocatorWrapperFlags} \
                --prefix PATH : ${gsettingsRuntime.binPath} \
                --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps} \
                --prefix XDG_DATA_DIRS : ${gsettingsRuntime.schemaDir} \
                --prefix XDG_DATA_DIRS : ${soundThemeShare} \
                --prefix GIO_EXTRA_MODULES : ${gsettingsRuntime.gioModules}
            '';
            postFixup = ''
              patchelf --add-rpath ${pkgs.lib.makeLibraryPath runtimeDlopenLibraries} $out/libexec/sottomano
            '';
            passthru = { inherit compositors; };
            meta.mainProgram = "sottomano";
            meta.platforms = pkgs.lib.platforms.linux;
            meta.license = pkgs.lib.licenses.gpl3Plus;
          }
        );

      sottomano = pkgs.lib.makeOverridable mkSottomano { };

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

      documentationSite = import ./nix/documentation-site.nix { inherit pkgs readmeRecordings; };

      vmTests = import ./vm-tests {
        sottomanoNixosModule = self.nixosModules.default;
        sottomanoPackage = sottomano;
      };
    in
    {
      packages.${system} = {
        default = sottomano;
        sottomano = sottomano;
        deps = showcaseArtifacts;
        workspace-dependencies = workspaceArtifacts;
        readme-recordings = readmeRecordings;
        documentation-site = documentationSite;
      };

      apps.${system} = import ./nix/ci-tasks.nix {
        inherit pkgs;
        mkApp = drv: {
          type = "app";
          program = pkgs.lib.getExe drv;
        };
        cargoToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        preCommitInputs = formatters;
      };

      checks.${system} = {
        documentation-site = documentationSite;
        tests = craneLib.cargoTest (
          workspaceArgs
          // showcaseFonts
          // {
            cargoArtifacts = workspaceArtifacts;
            doInstallCargoArtifacts = false;
            RUST_BACKTRACE = "1";
            RUST_TEST_THREADS = "1";
          }
        );
        clippy = craneLib.cargoClippy (
          workspaceArgs
          // {
            cargoArtifacts = workspaceArtifacts;
            cargoClippyExtraArgs = "--all-targets";
          }
        );
      }
      // pkgs.lib.listToAttrs (
        map (compositor: {
          name = "clippy-only-${compositor}";
          value = craneLib.cargoClippy (
            workspaceArgs
            // {
              pname = "sottomano-only-${compositor}";
              cargoArtifacts = workspaceArtifacts;
              cargoExtraArgs = "--locked -p sottomano-shell --no-default-features --features ${compositor}";
              cargoClippyExtraArgs = "--all-targets";
            }
          );
        }) allCompositors
      )
      // vmTests.checksFor pkgs;

      homeModules.default = {
        imports = [
          niri-hm.homeModules.default
          homeModule
        ];
      };

      nixosModules.default = import ./nix/nixos-module.nix { inherit self; };

      devShells.${system}.default = craneLib.devShell {
        packages =
          commonArgs.nativeBuildInputs
          ++ commonArgs.buildInputs
          ++ [
            pkgs.glib.bin
            pkgs.mdbook
          ]
          ++ formatters
          ++ runtimeDeps;
        env = allocatorEnv // {
          LD_LIBRARY_PATH = devShellLibraryPath;
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
