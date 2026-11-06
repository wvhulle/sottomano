{ pkgs, ... }:
{
  imports = [ ./session-target.nix ];

  programs.hyprland.enable = true;

  environment.etc."sottomano-test/hyprland.conf".text = ''
    monitor = Virtual-1, 1280x800, 0x0, 1
    misc {
        disable_hyprland_logo = true
        disable_splash_rendering = true
    }
    ecosystem {
        no_update_news = true
        no_donation_nag = true
    }
    exec-once = ${pkgs.dbus}/bin/dbus-update-activation-environment --systemd WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE XDG_CURRENT_DESKTOP && systemctl --user start compositor-test-session.target
  '';

  environment.sessionVariables.XDG_CURRENT_DESKTOP = "Hyprland";

  services.greetd = {
    enable = true;
    settings = {
      default_session.command = "Hyprland --config /etc/sottomano-test/hyprland.conf";
      initial_session = {
        command = "Hyprland --config /etc/sottomano-test/hyprland.conf";
        user = "session-user";
      };
    };
  };
}
