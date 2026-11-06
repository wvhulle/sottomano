{ pkgs, ... }:
{
  programs.niri = {
    enable = true;
    package = pkgs.niri.overrideAttrs (previousAttrs: {
      postPatch = (previousAttrs.postPatch or "") + ''
        substituteInPlace src/backend/tty.rs \
          --replace-fail '!egl_device.is_software(),' 'true,' \
          --replace-fail '.unwrap_or(node);' '.unwrap_or_else(|| node.node_with_type(NodeType::Render).and_then(Result::ok).unwrap_or(node));'
      '';
      doCheck = false;
    });
  };

  environment.etc."sottomano-test/niri.kdl".text = ''
    output "Virtual-1" {
        mode "1280x800"
    }
    hotkey-overlay {
        skip-at-startup
    }
    include "/home/session-user/.config/sottomano/niri.kdl" optional=true
  '';

  environment.sessionVariables.NIRI_CONFIG = "/etc/sottomano-test/niri.kdl";

  services.greetd = {
    enable = true;
    settings = {
      default_session.command = "niri-session";
      initial_session = {
        command = "niri-session";
        user = "session-user";
      };
    };
  };
}
