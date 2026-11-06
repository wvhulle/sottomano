{
  systemd.user.targets.compositor-test-session = {
    bindsTo = [ "graphical-session.target" ];
    wants = [ "graphical-session.target" ];
    after = [ "graphical-session-pre.target" ];
  };
}
