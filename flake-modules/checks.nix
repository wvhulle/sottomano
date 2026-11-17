{ config, ... }:
let
  supportedCompositors = config.sottomano.compositors;
in
{
  perSystem =
    { config, pkgs, ... }:
    let
      inherit (config.sottomano) cargoBuild;
      inherit (cargoBuild) craneLib workspaceArgs workspaceArtifacts;

      clippyForCompositor = compositor: {
        name = "cargo-clippy-${compositor}";
        value = craneLib.cargoClippy (
          workspaceArgs
          // {
            pname = "sottomano-only-${compositor}";
            cargoArtifacts = workspaceArtifacts;
            cargoExtraArgs = "--locked -p sottomano-shell --no-default-features --features ${compositor}";
            cargoClippyExtraArgs = "--all-targets";
          }
        );
      };
    in
    {
      packages.cargo-dependencies = workspaceArtifacts;

      checks = {
        cargo-test = craneLib.cargoTest (
          workspaceArgs
          // cargoBuild.showcaseFonts
          // {
            cargoArtifacts = workspaceArtifacts;
            doInstallCargoArtifacts = false;
            RUST_BACKTRACE = "1";
            RUST_TEST_THREADS = "1";
          }
        );
        cargo-clippy = craneLib.cargoClippy (
          workspaceArgs
          // {
            cargoArtifacts = workspaceArtifacts;
            cargoClippyExtraArgs = "--all-targets";
          }
        );
      }
      // pkgs.lib.listToAttrs (map clippyForCompositor supportedCompositors);
    };
}
