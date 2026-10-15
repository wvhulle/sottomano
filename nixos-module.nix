{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.vetrata;
  wireguardConfigured =
    config.networking.wireguard.enable || config.networking.wg-quick.interfaces != { };
in
{
  options.programs.vetrata = {
    enable = lib.mkEnableOption ''
      the system-level pieces vetrata widgets depend on: fwupd for the firmware
      upgrade reading, UPower for the power group, power-profiles-daemon for
      the power profile, the drivetemp kernel module for SATA drive
      temperatures and, when WireGuard is configured, a wheel-only `wg`
      wrapper with CAP_NET_ADMIN so the tunnel widget can run `wg show`.
      Services are set with mkDefault, so a host running tlp or auto-cpufreq
      can still disable power-profiles-daemon. The bar itself is installed per
      user through vetrata's home-manager module
    '';

    autoBrightness.enable = lib.mkEnableOption ''
      webcam-based automatic brightness through clight, which drives the
      monitor group's auto toggle and, through clightd, its backlight slider.
      clight's gamma, dimmer, DPMS, keyboard and screen modules are disabled by
      default because vetrata owns colour temperature and the idle timers
    '';

    cpuPowerReading.enable =
      lib.mkEnableOption ''
        reading CPU package and core power: a udev rule makes the RAPL
        `energy_uj` counters world-readable so the CPU tooltip can show live
        watts. RAPL readability is the PLATYPUS power side channel, which lets
        a local process infer data from fine-grained energy readings; turn
        this off on machines shared with untrusted users
      ''
      // {
        default = true;
      };

    cacheCounters.enable =
      lib.mkEnableOption ''
        counting L3 lookups and misses per AMD chiplet: loads `amd_uncore` and
        sets `kernel.perf_event_paranoid` to 0, which lets every local user
        count CPU-wide hardware events, so the CPU tooltip can show each
        chiplet's L3 hit rate. Defaults to on for hosts that load AMD
        microcode
      ''
      // {
        default = config.hardware.cpu.amd.updateMicrocode;
        defaultText = lib.literalExpression "config.hardware.cpu.amd.updateMicrocode";
      };

    x3dPreferenceSwitching.enable =
      lib.mkEnableOption ''
        switching the X3D scheduling preference (cache or frequency) from the
        bar: a udev rule makes `amd_x3d_mode` writable by the wheel group. The
        setting is a scheduler hint the kernel driver validates, accepts only
        `cache` or `frequency`, and resets at reboot; the rule matches no
        device on CPUs without the amd_x3d_vcache driver
      ''
      // {
        default = true;
      };

    journalReaders = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "alice" ];
      description = ''
        Users added to the systemd-journal group so the systemd and journal
        event groups can read the system journal. Members of wheel can already
        read it.
      '';
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        services = {
          fwupd.enable = lib.mkDefault true;
          upower.enable = lib.mkDefault true;
          power-profiles-daemon.enable = lib.mkDefault true;
        };

        boot.kernelModules = [ "drivetemp" ];

        users.users = lib.genAttrs cfg.journalReaders (_: {
          extraGroups = [ "systemd-journal" ];
        });
      }

      (lib.mkIf cfg.cpuPowerReading.enable {
        services.udev.extraRules = ''
          ACTION=="add", SUBSYSTEM=="powercap", KERNEL=="intel-rapl:*", RUN+="${pkgs.coreutils}/bin/chmod 0444 /sys%p/energy_uj"
        '';
      })

      (lib.mkIf cfg.cacheCounters.enable {
        boot.kernelModules = [ "amd_uncore" ];
        boot.kernel.sysctl."kernel.perf_event_paranoid" = lib.mkDefault 0;
      })

      (lib.mkIf cfg.x3dPreferenceSwitching.enable {
        services.udev.extraRules = ''
          ACTION=="add|bind", SUBSYSTEM=="platform", DRIVER=="amd_x3d_vcache", RUN+="${pkgs.coreutils}/bin/chgrp wheel /sys%p/amd_x3d_mode", RUN+="${pkgs.coreutils}/bin/chmod 0664 /sys%p/amd_x3d_mode"
        '';
      })

      (lib.mkIf wireguardConfigured {
        security.wrappers.wg = {
          source = "${pkgs.wireguard-tools}/bin/wg";
          capabilities = "cap_net_admin+ep";
          owner = "root";
          group = "wheel";
          permissions = "u+rx,g+rx";
        };
      })

      (lib.mkIf cfg.autoBrightness.enable {
        nixpkgs.overlays = [
          (_: prev: {
            clight = prev.clight.overrideAttrs (old: {
              postInstall = (old.postInstall or "") + ''
                sed -i 's|^Exec=.*|SystemdService=clight.service|' \
                  $out/share/dbus-1/services/org.clight.clight.service
              '';
            });
          })
        ];

        services.clight = {
          enable = true;
          settings = {
            gamma.disabled = lib.mkDefault true;
            dimmer.disabled = lib.mkDefault true;
            dpms.disabled = lib.mkDefault true;
            keyboard.disabled = lib.mkDefault true;
            screen.disabled = lib.mkDefault true;
          };
        };

        systemd.user.services.clight.serviceConfig = {
          Type = "dbus";
          BusName = "org.clight.clight";
        };

        services.geoclue2.enable = lib.mkDefault true;
        location.provider = lib.mkDefault "geoclue2";

        environment.etc."xdg/autostart/clight.desktop".text = lib.generators.toINI { } {
          "Desktop Entry" = {
            Type = "Application";
            Name = "Clight";
            Hidden = true;
          };
        };
      })
    ]
  );
}
