{
  perSystem =
    { config, pkgs, ... }:
    let
      inherit (pkgs) lib;

      exportBuiltPathsHook = lib.getExe (
        pkgs.writers.writeNuBin "export-built-paths-hook" {
          makeWrapperArgs = [
            "--add-flags"
            "export-built-paths-hook"
          ];
        } (builtins.readFile ../ci/nix-built-paths.nu)
      );

      ciScriptShell = pkgs.mkShellNoCC {
        packages = [
          pkgs.nushell
          pkgs.gitMinimal
          pkgs.tea
          pkgs.convco
          pkgs.cachix
        ];
        env.EXPORT_HOOK_PROGRAM = exportBuiltPathsHook;
      };
    in
    {
      devShells = {
        ci = ciScriptShell;
        ci-cargo = pkgs.mkShellNoCC {
          inputsFrom = [ ciScriptShell ];
          packages = [
            config.sottomano.cargoBuild.rustToolchain
            pkgs.release-plz
          ];
          env.EXPORT_HOOK_PROGRAM = exportBuiltPathsHook;
        };
      };
    };
}
