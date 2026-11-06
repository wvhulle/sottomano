{ pkgs, sottomanoPackage, ... }:
{
  system.stateVersion = "26.05";

  users.users.session-user = {
    isNormalUser = true;
    uid = 1000;
    password = "session-password";
    extraGroups = [ "wheel" ];
  };

  programs.sottomano.enable = true;

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
