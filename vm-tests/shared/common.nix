{ pkgs, sottomanoPackage, ... }:
let
  ocrReadableBarConfig = pkgs.writeText "sottomano.toml" ''
    [theme.palette]
    background = "#ffffff"
    background_weak = "#f0f0f0"
    background_strong = "#d0d0d0"
    text = "#000000"
    secondary = "#000000"
    primary = "#000000"

    [theme.font]
    text_size = "28px"

    [desktop_panels]
    height = "48px"
  '';
in
{
  system.stateVersion = "26.05";

  users.users.session-user = {
    isNormalUser = true;
    uid = 1000;
    password = "session-password";
    extraGroups = [ "wheel" ];
  };

  programs.sottomano.enable = true;

  systemd.tmpfiles.rules = [
    "d /home/session-user/.config 0755 session-user users -"
    "L+ /home/session-user/.config/sottomano.toml - - - - ${ocrReadableBarConfig}"
  ];

  hardware.graphics.enable = true;

  environment.systemPackages = [
    sottomanoPackage
    pkgs.foot
    pkgs.jq
  ];

  fonts.packages = [
    pkgs.ibm-plex
    pkgs.dejavu_fonts
  ];

  systemd.user.services.sottomano = {
    partOf = [ "graphical-session.target" ];
    after = [ "graphical-session.target" ];
    wantedBy = [ "graphical-session.target" ];
    serviceConfig = {
      ExecStart = "${sottomanoPackage}/bin/sottomano";
      Restart = "on-failure";
      RestartSec = 2;
    };
  };

  virtualisation = {
    memorySize = 4096;
    cores = 4;
    qemu.options = [
      "-vga none"
      "-device virtio-gpu-pci,xres=1280,yres=800"
    ];
  };
}
