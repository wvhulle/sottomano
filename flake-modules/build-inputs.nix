{ inputs, lib, ... }:
{
  options.sottomano.compositors = lib.mkOption {
    type = lib.types.listOf lib.types.str;
    readOnly = true;
    default = [
      "niri"
      "sway"
      "hyprland"
    ];
    description = "Compositors sottomano supports, one cargo feature each.";
  };

  config.perSystem =
    { pkgs, inputs', ... }:
    let
      rustToolchain =
        (inputs.rust-overlay.lib.mkRustBin { } pkgs).fromRustupToolchainFile
          ../rust-toolchain.toml;

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain (
        toolchainPkgs:
        (inputs.rust-overlay.lib.mkRustBin { } toolchainPkgs).fromRustupToolchainFile ../rust-toolchain.toml
      );

      workspaceMemberDirectories =
        map (member: ../. + "/${member}")
          (lib.importTOML ../Cargo.toml).workspace.members;

      cargoWorkspaceFileset = lib.fileset.unions (
        [
          ../Cargo.toml
          ../Cargo.lock
          ../.cargo
        ]
        ++ workspaceMemberDirectories
      );

      src = lib.fileset.toSource {
        root = ../.;
        fileset = lib.fileset.union cargoWorkspaceFileset ../.clippy.toml;
      };

      runtimeDlopenLibraries = [
        pkgs.wayland
        pkgs.libxkbcommon
        pkgs.libGL
        pkgs.vulkan-loader
      ];

      allocatorEnv = {
        MALLOC_ARENA_MAX = "2";
        GLIBC_TUNABLES = "glibc.malloc.mmap_threshold=131072:glibc.malloc.trim_threshold=131072";
      };

      iconAssetEnv = {
        WEATHER_ICONS_SVG = "${pkgs.weather-icons.src}/svg";
        PHOSPHOR_SVG = "${inputs.phosphor-core}/assets";
        SIMPLE_ICONS_SVG = "${inputs.simple-icons}/icons";
      };

      showcaseAppIcons = "${pkgs.papirus-icon-theme}/share/icons/Papirus/64x64/apps";

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

      linkedSystemLibraryEnv = {
        AWS_LC_SYS_USE_SYSTEM = "1";
        ZSTD_SYS_USE_PKG_CONFIG = "1";
      };

      dependencyBuildArgs = linkedSystemLibraryEnv // {
        inherit src;
        cargoVendorDir = vendoredCargoDeps;
        version = (lib.importTOML ../Cargo.toml).workspace.package.version;
        strictDeps = true;
        NIX_OUTPATH_USED_AS_RANDOM_SEED = "sottomanos";
        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.autoAddDriverRunpath
          pkgs.rustPlatform.bindgenHook
        ];
        buildInputs = runtimeDlopenLibraries ++ [
          pkgs.systemd
          pkgs.pipewire
          pkgs.openssl
          pkgs.aws-lc
          pkgs.libvirt
          pkgs.zstd
        ];
      };

      commonArgs = iconAssetEnv // dependencyBuildArgs;

      shellDependencyArtifacts = craneLib.buildDepsOnly (
        dependencyBuildArgs
        // {
          inherit (shellArgs) pname cargoExtraArgs;
          cargoCheckCommand = "true";
          doCheck = false;
        }
      );

      showcaseDependencyArtifacts = craneLib.mkCargoDerivation (
        dependencyBuildArgs
        // {
          pname = "${showcaseArgs.pname}-deps";
          src = craneLib.mkDummySrc { inherit src; };
          cargoArtifacts = shellDependencyArtifacts;
          buildPhaseCargoCommand = "cargoWithProfile build ${showcaseArgs.cargoExtraArgs}";
          doInstallCargoArtifacts = true;
        }
      );

      devDependencyArtifacts = craneLib.buildDepsOnly (
        dependencyBuildArgs
        // {
          pname = workspaceArgs.pname;
          inherit (workspaceArgs) cargoExtraArgs CARGO_PROFILE CARGO_PROFILE_DEV_DEBUG;
        }
      );

      shellArgs = commonArgs // {
        pname = "sottomano";
        cargoExtraArgs = "--locked -p sottomano-shell";
      };

      sourcesAffectingRecordings =
        let
          showcaseCrateSources = lib.fileset.toSource {
            root = ../.;
            fileset = lib.fileset.unions [
              (lib.fileset.difference cargoWorkspaceFileset (
                lib.fileset.unions [
                  ../sottomano-shell
                  ../xtask
                  ../sottomano-widget-showcase/tests
                ]
              ))
              ../sottomano-shell/Cargo.toml
              ../xtask/Cargo.toml
            ];
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

      showcaseFonts = {
        nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.fontconfig ];
        FONTCONFIG_FILE = pkgs.makeFontsConf {
          fontDirectories = [
            pkgs.ibm-plex
            pkgs.nerd-fonts.lilex
            pkgs.dejavu_fonts
          ];
          includes = [ ./fonts.conf.d ];
        };
      };
    in
    {
      options.sottomano = {
        cargoBuild = lib.mkOption {
          type = lib.types.lazyAttrsOf lib.types.raw;
          readOnly = true;
          description = "The Rust toolchain, crane library, cargo build arguments and dependency artifacts shared by every cargo derivation.";
        };
        desktopRuntime = lib.mkOption {
          type = lib.types.lazyAttrsOf lib.types.raw;
          readOnly = true;
          description = "Runtime libraries, tools and environment the bar needs, shared by the package wrapper and the dev shell.";
        };
      };

      config.sottomano = {
        cargoBuild = {
          inherit
            rustToolchain
            craneLib
            commonArgs
            shellArgs
            showcaseArgs
            workspaceArgs
            showcaseFonts
            ;
          shellArtifacts = shellDependencyArtifacts;
          workspaceArtifacts = devDependencyArtifacts;
          showcaseArtifacts = showcaseDependencyArtifacts;
        };

        desktopRuntime = {
          inherit
            runtimeDlopenLibraries
            allocatorEnv
            iconAssetEnv
            linkedSystemLibraryEnv
            showcaseAppIcons
            ;

          allocatorWrapperFlags = lib.concatStringsSep " " (
            lib.mapAttrsToList (name: value: "--set-default ${name} ${lib.escapeShellArg value}") allocatorEnv
          );

          runtimeDeps = [
            pkgs.fontconfig
            pkgs.iproute2
            pkgs.networkmanager
            pkgs.modemmanager
            pkgs.bluez
            pkgs.inxi
            pkgs.wireplumber
            pkgs.pipewire
            pkgs.openssl
            pkgs.wireguard-tools
            pkgs.tailscale
            pkgs.xdg-utils
            pkgs.wl-clipboard
            pkgs.systemd
            inputs'.umbra.packages.default
          ];

          gsettingsRuntime = {
            binPath = "${pkgs.glib.bin}/bin";
            schemaDir = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}";
            gioModules = "${pkgs.dconf.lib}/lib/gio/modules";
          };

          soundThemeShare = "${pkgs.sound-theme-freedesktop}/share";
        };
      };
    };
}
