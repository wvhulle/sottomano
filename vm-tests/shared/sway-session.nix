{ pkgs, ... }:
{
  imports = [ ./session-target.nix ];

  programs.sway.enable = true;

  environment.etc."sottomano-test/sway.conf".text = ''
    output Virtual-1 resolution 1280x800
    output * bg #202020 solid_color
    include /home/session-user/.config/sottomano/sway.conf
    exec ${pkgs.dbus}/bin/dbus-update-activation-environment --systemd WAYLAND_DISPLAY SWAYSOCK XDG_CURRENT_DESKTOP && systemctl --user start compositor-test-session.target
  '';

  environment.sessionVariables = {
    SWAYSOCK = "/tmp/sway-ipc.sock";
    WLR_RENDERER = "pixman";
    WLR_LIBINPUT_NO_DEVICES = "1";
    XDG_CURRENT_DESKTOP = "sway";
  };

  services.greetd = {
    enable = true;
    settings = {
      default_session.command = "sway --config /etc/sottomano-test/sway.conf";
      initial_session = {
        command = "sway --config /etc/sottomano-test/sway.conf";
        user = "session-user";
      };
    };
  };
}
