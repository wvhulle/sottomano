{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.sottomano;
  sottomanoPackage = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
in
{
  options.programs.sottomano = {
    enable = lib.mkEnableOption ''
      the system-level pieces sottomano widgets depend on: fwupd for the firmware
      upgrade reading, UPower for the power group, power-profiles-daemon for
      the power profile and the drivetemp kernel module for SATA drive
      temperatures. Services are set with mkDefault, so a host running tlp or auto-cpufreq
      can still disable power-profiles-daemon. The bar itself is installed per
      user through sottomano's home-manager module
    '';

    autoBrightness.enable = lib.mkEnableOption ''
      webcam-based automatic brightness through clight, which drives the
      monitor group's auto toggle and, through clightd, its backlight slider.
      clight's gamma, dimmer, DPMS, keyboard and screen modules are disabled by
      default because sottomano owns colour temperature and the idle timers
    '';

    cpuPowerReading.enable =
      lib.mkEnableOption ''
        reading CPU package and core power: a udev rule makes the RAPL
        `energy_uj` counters world-readable so the CPU power draw widget can
        show live watts. RAPL readability is the PLATYPUS power side channel, which lets
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
        count CPU-wide hardware events, so the CPU hover card can show each
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
        bar: a udev rule and a tmpfiles rule make `amd_x3d_mode` writable by
        the wheel group, at device bind, at boot and on every rebuild. The
        setting is a scheduler hint the kernel driver validates, accepts only
        `cache` or `frequency`, and resets at reboot; the rules match no
        device on CPUs without the amd_x3d_vcache driver
      ''
      // {
        default = true;
      };

    secretMemoryInspection.enable =
      lib.mkEnableOption ''
        naming the apps that stop hibernation by holding secret memory: installs
        a small helper with `cap_sys_ptrace` that reads `/proc/*/maps` and prints
        the PIDs mapping `memfd_secret` memory, so the Hibernate group can name
        them. The capability is on that one helper rather than the bar, and it
        takes no arguments. Any local user can run it and learn which PIDs hold
        secret memory, though never the contents. Without it the group only
        reports that the kernel disabled hibernation
      ''
      // {
        default = true;
      };

    nspawnContainerControllers = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "alice" ];
      description = ''
        Users allowed to start, stop and restart systemd-nspawn containers from
        the bar's container switches. A polkit rule grants them exactly those verbs
        on `container@*.service` (NixOS containers) and `systemd-nspawn@*.service`
        units, without a password prompt, and nothing else.
      '';
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

      (lib.mkIf (cfg.nspawnContainerControllers != [ ]) {
        security.polkit = {
          enable = true;
          extraConfig = ''
            polkit.addRule(function (action, subject) {
              var containerControllers = ${builtins.toJSON cfg.nspawnContainerControllers};
              var managedUnit = action.lookup("unit");
              var requestedVerb = action.lookup("verb");
              if (action.id == "org.freedesktop.systemd1.manage-units"
                  && managedUnit
                  && /^(container|systemd-nspawn)@[^/]+\.service$/.test(managedUnit)
                  && ["start", "stop", "restart"].indexOf(requestedVerb) >= 0
                  && containerControllers.indexOf(subject.user) >= 0) {
                return polkit.Result.YES;
              }
            });
          '';
        };
      })

      (lib.mkIf cfg.secretMemoryInspection.enable {
        security.wrappers.sottomano-secret-memory-holders = {
          source = "${sottomanoPackage}/bin/sottomano-secret-memory-holders";
          owner = "root";
          group = "root";
          capabilities = "cap_sys_ptrace+ep";
        };
      })

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
        systemd.tmpfiles.rules = [
          "z /sys/bus/platform/drivers/amd_x3d_vcache/*/amd_x3d_mode 0664 root wheel - -"
        ];
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
