{ config, self, ... }:
let
  supportedCompositors = config.sottomano.compositors;
in
{
  perSystem =
    { pkgs, self', ... }:
    let
      inherit (pkgs) lib;

      scenarioNames = lib.attrNames (
        lib.filterAttrs (
          entryName: entryKind:
          entryKind == "directory" && builtins.pathExists (../vm-tests + "/${entryName}/check.py")
        ) (builtins.readDir ../vm-tests)
      );

      machineForCompositor = compositorName: {
        imports = [
          self.nixosModules.default
          ../vm-tests/shared/common.nix
          ../vm-tests/shared/${compositorName}-session.nix
        ];
        _module.args.sottomanoPackage = self'.packages.sottomano;
        networking.hostName = "sottomano-${compositorName}";
      };

      scenarioTest =
        compositor: scenario:
        pkgs.testers.runNixOSTest {
          name = "sottomano-${compositor}-${scenario}";
          enableOCR = true;
          nodes.machine = machineForCompositor compositor;
          testScript = ''
            COMPOSITOR = "${compositor}"
          ''
          + builtins.readFile ../vm-tests/shared/compositor_session.py
          + builtins.readFile ../vm-tests/${scenario}/check.py;
        };

      scenarioTestsByCompositor = lib.genAttrs supportedCompositors (
        compositor: lib.genAttrs scenarioNames (scenarioTest compositor)
      );
    in
    {
      legacyPackages.vm-tests = scenarioTestsByCompositor;

      checks = lib.mapAttrs' (
        compositor: scenarioTests:
        lib.nameValuePair "vm-${compositor}" (
          pkgs.linkFarmFromDrvs "sottomano-vm-tests-${compositor}" (lib.attrValues scenarioTests)
        )
      ) scenarioTestsByCompositor;
    };
}
