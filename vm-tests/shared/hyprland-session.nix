{ pkgs, ... }:
{
  imports = [ ./session-target.nix ];

  programs.hyprland.enable = true;

  environment.etc."sottomano-test/hyprland.lua".text = ''
    hl.monitor({ output = "Virtual-1", mode = "1280x800", position = "0x0", scale = 1 })

    hl.config({
        misc = {
            disable_hyprland_logo = true,
            disable_splash_rendering = true,
        },
        ecosystem = {
            no_update_news = true,
            no_donation_nag = true,
        },
    })

    hl.on("hyprland.start", function()
        hl.exec_cmd("${pkgs.dbus}/bin/dbus-update-activation-environment --systemd WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP && systemctl --user start compositor-test-session.target")
    end)

    local sottomano_rules = os.getenv("HOME") .. "/.config/sottomano/hyprland.lua"
    local sottomano_rules_file = io.open(sottomano_rules)
    if sottomano_rules_file then
        sottomano_rules_file:close()
        dofile(sottomano_rules)
    end
  '';

  environment.sessionVariables.XDG_CURRENT_DESKTOP = "Hyprland";

  services.greetd = {
    enable = true;
    settings = {
      default_session.command = "start-hyprland -- --config /etc/sottomano-test/hyprland.lua";
      initial_session = {
        command = "start-hyprland -- --config /etc/sottomano-test/hyprland.lua";
        user = "session-user";
      };
    };
  };
}
