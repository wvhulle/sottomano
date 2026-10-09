{
  config,
  lib,
  ...
}:
let
  cfg = config.programs.vetrata;
in
{
  options.programs.vetrata.enable = lib.mkEnableOption ''
    the system services whose D-Bus interfaces vetrata widgets read: fwupd for
    the firmware upgrade reading, UPower for the battery group and
    power-profiles-daemon for the power profile. Each is set with mkDefault, so
    a host running tlp or auto-cpufreq can still disable power-profiles-daemon.
    The bar itself is installed per user through vetrata's home-manager module
  '';

  config = lib.mkIf cfg.enable {
    services = {
      fwupd.enable = lib.mkDefault true;
      upower.enable = lib.mkDefault true;
      power-profiles-daemon.enable = lib.mkDefault true;
    };
  };
}
