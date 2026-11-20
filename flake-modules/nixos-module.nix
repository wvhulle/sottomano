{ moduleWithSystem, ... }:
{
  flake.nixosModules.default = moduleWithSystem (
    { config }:
    let
      sottomanoPackage = config.packages.default;
    in
    {
      config,
      lib,
      pkgs,
      ...
    }:
    let
      cfg = config.programs.sottomano;
      containerControllerGroup = "sottomano-containers";
      hibernateDelayControllerGroup = "sottomano-hibernate-delay";
      hibernateDelayDropIn = "/etc/systemd/sleep.conf.d/50-sottomano.conf";
      writeHibernateDelay = pkgs.writeShellScript "sottomano-write-hibernate-delay" ''
        set -euo pipefail
        requested_instance="$1"
        drop_in_directory="$(${pkgs.coreutils}/bin/dirname ${hibernateDelayDropIn})"
        if ! [[ "$requested_instance" =~ ^[1-9][0-9]{0,5}$ ]]; then
          echo "hibernate delay must be whole seconds, got: $requested_instance" >&2
          exit 1
        fi
        if (( requested_instance < ${toString cfg.hibernateDelayRange.shortestSeconds} || requested_instance > ${toString cfg.hibernateDelayRange.longestSeconds} )); then
          echo "hibernate delay $requested_instance s is outside the allowed range" >&2
          exit 1
        fi
        ${pkgs.coreutils}/bin/mkdir -p "$drop_in_directory"
        staged_drop_in="$(${pkgs.coreutils}/bin/mktemp "$drop_in_directory/.50-sottomano.XXXXXX")"
        printf '[Sleep]\nHibernateDelaySec=%ss\n' "$requested_instance" > "$staged_drop_in"
        ${pkgs.coreutils}/bin/chmod 0644 "$staged_drop_in"
        ${pkgs.coreutils}/bin/mv -f "$staged_drop_in" ${hibernateDelayDropIn}
      '';
    in
    {
      imports = [
        (lib.mkRenamedOptionModule
          [ "programs" "sottomano" "nspawnContainerControllers" ]
          [ "programs" "sottomano" "containerControllers" ]
        )
      ];

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

        containerControllers = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [ ];
          example = [ "alice" ];
          description = ''
            Users added to the `sottomano-containers` group, whose members may
            start, stop and restart containers and microVMs from the bar's
            switches. A polkit rule grants the group exactly those verbs on
            `container@*.service` (NixOS containers), `systemd-nspawn@*.service`,
            `microvm@*.service` and `waydroid-container.service` units, without a
            password prompt, and nothing else. Any process running as a member can
            use it, so a member's compromised app could stop these units. Group
            changes apply at the next login.
          '';
        };

        hibernateDelayControllers = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [ ];
          example = [ "alice" ];
          description = ''
            Users added to the `sottomano-hibernate-delay` group, whose members may
            set how long a suspended machine waits before hibernating (systemd's
            `HibernateDelaySec`) from the bar's Hibernate group. A polkit rule lets the
            group start only `sottomano-hibernate-delay@<seconds>.service`, without a
            password prompt. The unit accepts whole seconds within
            `hibernateDelayRange` and writes `${hibernateDelayDropIn}`; delete that file
            to return to the NixOS value. The drop-in
            overrides `systemd.sleep.settings.Sleep.HibernateDelaySec` and applies to
            every suspend-then-hibernate, including lid close. Any process running as
            a member can change the delay. Group changes apply at the next login.
          '';
        };

        hibernateDelayRange = {
          shortestSeconds = lib.mkOption {
            type = lib.types.ints.positive;
            default = 10;
            description = "Shortest hibernate delay the helper unit accepts.";
          };
          longestSeconds = lib.mkOption {
            type = lib.types.ints.positive;
            default = 86400;
            description = "Longest hibernate delay the helper unit accepts.";
          };
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

          {
            users.groups.${containerControllerGroup} = { };

            users.users = lib.genAttrs cfg.containerControllers (_: {
              extraGroups = [ containerControllerGroup ];
            });

            security.polkit = {
              enable = true;
              extraConfig = ''
                polkit.addRule(function (action, subject) {
                  var managedUnit = action.lookup("unit");
                  var requestedVerb = action.lookup("verb");
                  if (action.id == "org.freedesktop.systemd1.manage-units"
                      && managedUnit
                      && /^((container|systemd-nspawn|microvm)@[^/]+|waydroid-container)\.service$/.test(managedUnit)
                      && ["start", "stop", "restart"].indexOf(requestedVerb) >= 0
                      && subject.isInGroup("${containerControllerGroup}")) {
                    return polkit.Result.YES;
                  }
                });
              '';
            };
          }

          (lib.mkIf (cfg.hibernateDelayControllers != [ ]) {
            users.groups.${hibernateDelayControllerGroup} = { };

            users.users = lib.genAttrs cfg.hibernateDelayControllers (_: {
              extraGroups = [ hibernateDelayControllerGroup ];
            });

            systemd.services."sottomano-hibernate-delay@" = {
              description = "Set the suspend-then-hibernate delay to %i";
              serviceConfig = {
                Type = "oneshot";
                ExecStart = "${writeHibernateDelay} %i";
                ProtectSystem = "strict";
                ReadWritePaths = [ "/etc/systemd" ];
                ProtectHome = true;
                PrivateNetwork = true;
                PrivateTmp = true;
                NoNewPrivileges = true;
                CapabilityBoundingSet = "";
              };
            };

            security.polkit = {
              enable = true;
              extraConfig = ''
                polkit.addRule(function (action, subject) {
                  var managedUnit = action.lookup("unit");
                  if (action.id == "org.freedesktop.systemd1.manage-units"
                      && managedUnit
                      && /^sottomano-hibernate-delay@[1-9][0-9]{0,5}\.service$/.test(managedUnit)
                      && action.lookup("verb") == "start"
                      && subject.isInGroup("${hibernateDelayControllerGroup}")) {
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
  );
}
