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
        networking.hostName = "sottomano-${compositorName}";
      };

      scenarioTest =
        compositor: scenario:
        pkgs.testers.runNixOSTest {
          name = "sottomano-${compositor}-${scenario}";
          enableOCR = true;
          nodes.machine = machineForCompositor compositor;
          node.specialArgs.sottomanoPackage = self'.packages.sottomano;
          testScript = ''
            COMPOSITOR = "${compositor}"
          ''
          + builtins.readFile ../vm-tests/shared/compositor_session.py
          + builtins.readFile ../vm-tests/${scenario}/check.py;
        };

      hibernateDelayUnitTest = pkgs.testers.runNixOSTest {
        name = "sottomano-hibernate-delay-unit";
        nodes.machine = {
          imports = [ self.nixosModules.default ];
          programs.sottomano = {
            enable = true;
            hibernateDelayControllers = [ "alice" ];
          };
          users.users = {
            alice.isNormalUser = true;
            bob.isNormalUser = true;
          };
        };
        testScript = builtins.readFile ../vm-tests/hibernate-delay-unit/test_script.py;
      };

      scenarioTestsByCompositor = lib.genAttrs supportedCompositors (
        compositor: lib.genAttrs scenarioNames (scenarioTest compositor)
      );
    in
    {
      legacyPackages.vm-tests = scenarioTestsByCompositor // {
        hibernate-delay-unit = hibernateDelayUnitTest;
      };

      checks = {
        vm-hibernate-delay-unit = hibernateDelayUnitTest;
      }
      // lib.mapAttrs' (
        compositor: scenarioTests:
        lib.nameValuePair "vm-${compositor}" (
          pkgs.linkFarmFromDrvs "sottomano-vm-tests-${compositor}" (lib.attrValues scenarioTests)
        )
      ) scenarioTestsByCompositor;
    };
}
