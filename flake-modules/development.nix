{
  perSystem =
    { config, pkgs, ... }:
    let
      inherit (pkgs) lib;
      inherit (config.sottomano) cargoBuild desktopRuntime;
      inherit (cargoBuild) commonArgs;
      inherit (desktopRuntime) gsettingsRuntime soundThemeShare;
    in
    {
      apps.icon-svg-exports.program = pkgs.writeShellScriptBin "icon-svg-exports" ''
        cat <<'EXPORTS'
        ${lib.concatMapAttrsStringSep "\n" (
          name: value: "export ${name}=${lib.escapeShellArg value}"
        ) desktopRuntime.iconAssetEnv}
        EXPORTS
      '';

      devShells.default = cargoBuild.craneLib.devShell {
        packages =
          commonArgs.nativeBuildInputs
          ++ commonArgs.buildInputs
          ++ [
            pkgs.glib.bin
            pkgs.mdbook
            pkgs.mdbook-mermaid
            config.treefmt.build.wrapper
          ]
          ++ config.pre-commit.settings.enabledPackages
          ++ desktopRuntime.runtimeDeps;
        env =
          desktopRuntime.allocatorEnv
          // desktopRuntime.iconAssetEnv
          // desktopRuntime.linkedSystemLibraryEnv
          // {
            LD_LIBRARY_PATH = "${lib.makeLibraryPath (lib.remove pkgs.openssl commonArgs.buildInputs)}:${pkgs.addDriverRunpath.driverLink}/lib";
            SHOWCASE_APP_ICONS = desktopRuntime.showcaseAppIcons;
          };
        shellHook = ''
          export XDG_DATA_DIRS="${gsettingsRuntime.schemaDir}:${soundThemeShare}''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
          export GIO_EXTRA_MODULES="${gsettingsRuntime.gioModules}''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
          ${config.pre-commit.installationScript}
        '';
      };
    };
}
