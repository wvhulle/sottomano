{ sottomanoNixosModule, sottomanoPackage }:
let
  compositorNames = [
    "niri"
    "sway"
    "hyprland"
  ];

  scenarioNames = [
    "panel-mapped"
    "styling-rules"
  ];

  machineFor = compositorName: {
    imports = [
      sottomanoNixosModule
      ./shared/common.nix
      ./shared/${compositorName}-session.nix
    ];
    _module.args = { inherit sottomanoPackage; };
    networking.hostName = "sottomano-${compositorName}";
  };

  checkFor =
    pkgs:
    { compositor, scenario }:
    pkgs.lib.nameValuePair "vm-${compositor}-${scenario}" (
      pkgs.testers.runNixOSTest {
        name = "sottomano-${compositor}-${scenario}";
        enableOCR = true;
        nodes.machine = machineFor compositor;
        testScript = ''
          COMPOSITOR = "${compositor}"
        ''
        + builtins.readFile ./shared/compositor_session.py
        + builtins.readFile ./${scenario}/check.py;
      }
    );
in
{
  checksFor =
    pkgs:
    builtins.listToAttrs (
      map (checkFor pkgs) (
        pkgs.lib.cartesianProduct {
          compositor = compositorNames;
          scenario = scenarioNames;
        }
      )
    );
}
