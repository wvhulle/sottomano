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
      controlGroupNames = {
        containerControl = "sottomano-containers";
        hibernateDelayControl = "sottomano-hibernate-delay";
        tailnetPolicyReaders = "sottomano-tailnet-policy";
      };
      tailnetPolicyRuntimeDirectory = "sottomano-tailnet-policy";
      tailnetPolicyTokenPath = "/run/${tailnetPolicyRuntimeDirectory}/access-token";
      tailnetPolicyTokenServiceName = "sottomano-tailnet-policy-token";
      oauthClientCredentialName = "oauth-client";
      mintTailnetPolicyTokenCommand = lib.concatStringsSep " " [
        (lib.getExe pkgs.nushell)
        "--no-config-file"
        "${./mint-tailnet-policy-token.nu}"
        "/run/credentials/${tailnetPolicyTokenServiceName}.service/${oauthClientCredentialName}"
        tailnetPolicyTokenPath
      ];
      hibernateDelayDropIn = "/etc/systemd/sleep.conf.d/50-sottomano.conf";
    in
    {
      imports = [
        (lib.mkRemovedOptionModule [ "programs" "sottomano" "nspawnContainerControllers" ] ''
          Set programs.sottomano.containerControl.enable and add each user to the group named by
          programs.sottomano.groups.containerControl.
        '')
        (lib.mkRemovedOptionModule [ "programs" "sottomano" "containerControllers" ] ''
          Set programs.sottomano.containerControl.enable and add each user to the group named by
          programs.sottomano.groups.containerControl.
        '')
        (lib.mkRemovedOptionModule [ "programs" "sottomano" "hibernateDelayControllers" ] ''
          Set programs.sottomano.hibernateDelayControl.enable and add each user to the group named by
          programs.sottomano.groups.hibernateDelayControl.
        '')
        (lib.mkRemovedOptionModule [ "programs" "sottomano" "journalReaders" ] ''
          Add each user to the systemd-journal group (users.users.<name>.extraGroups) instead.
        '')
      ];

      options.programs.sottomano = {
        enable = lib.mkEnableOption ''
          the system services sottomano widgets read: fwupd for firmware upgrades,
          UPower for the power group, power-profiles-daemon for the power profile and
          the drivetemp kernel module for SATA drive temperatures. Services are set with
          mkDefault, so a host running tlp or auto-cpufreq can still disable
          power-profiles-daemon. Features that grant users extra rights each have their
          own toggle below. The bar itself is installed per user through sottomano's
          home-manager module
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

        containerControl.enable = lib.mkEnableOption ''
          starting, stopping and restarting containers and microVMs from the bar's
          switches. Creates the group named by `groups.containerControl`; a polkit rule
          grants its members exactly those verbs on `container@*.service` (NixOS
          containers), `systemd-nspawn@*.service`, `microvm@*.service` and
          `waydroid-container.service`, without a password prompt, and nothing else.
          Add users with `users.users.<name>.extraGroups`. Any process running as a
          member can use it, so a member's compromised app could stop these units.
          Group changes apply at the next login
        '';

        hibernateDelayControl.enable = lib.mkEnableOption ''
          setting how long a suspended machine waits before hibernating (systemd's
          `HibernateDelaySec`) from the bar's Hibernate group. Creates the group named by
          `groups.hibernateDelayControl`; a polkit rule lets its members start only
          `sottomano-hibernate-delay@<seconds>.service`, without a password prompt. The
          unit accepts whole seconds within `hibernateDelayRange` and writes
          `${hibernateDelayDropIn}`; delete that file to return to the NixOS value. The
          drop-in overrides `systemd.sleep.settings.Sleep.HibernateDelaySec` and applies
          to every suspend-then-hibernate, including lid close. Add users with
          `users.users.<name>.extraGroups`; group changes apply at the next login
        '';

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

        tailnetPolicyToken = {
          enable = lib.mkEnableOption ''
            minting a short-lived Tailscale API token from an OAuth client every 30
            minutes, so the Tailscale hover card can read the tailnet policy file
            without the desktop user holding the OAuth secret. The token lands in
            `tailnetPolicyToken.path`, readable by the group named by
            `groups.tailnetPolicyReaders`. Point
            `services.sottomano.settings.secrets.tailscale_api_token` at that path and
            add the user to the group. Give the OAuth client only the `policy_file:read`
            scope
          '';

          oauthClientEnvironmentFile = lib.mkOption {
            type = lib.types.str;
            example = "/run/agenix/tailscale-policy-reader-oauth-client";
            description = ''
              Path to a systemd environment file with `TAILSCALE_OAUTH_CLIENT_ID=` and
              `TAILSCALE_OAUTH_CLIENT_SECRET=`. Pass a runtime path such as an agenix or
              sops-nix secret, never a Nix path, which would copy the secret into the store.
            '';
          };

          timerWantedBy = lib.mkOption {
            type = lib.types.listOf lib.types.str;
            default = [ "timers.target" ];
            example = [ "network-online.target" ];
            description = ''
              Units that start the minting timer, which also stops when they stop. The
              timer mints once each time it starts and then every 30 minutes. Bind it to
              a target that is active only while the machine is online, so it never mints
              without a network, for example right after resuming from suspend.
            '';
          };

          path = lib.mkOption {
            type = lib.types.str;
            readOnly = true;
            default = tailnetPolicyTokenPath;
            description = "Where the minted token is written.";
          };

          mintCommand = lib.mkOption {
            type = lib.types.str;
            readOnly = true;
            default = mintTailnetPolicyTokenCommand;
            description = ''
              The exact command line of the minting service, for firewalls such as
              OpenSnitch that can match on `process.command`.
            '';
          };
        };

        groups = lib.mkOption {
          type = lib.types.attrsOf lib.types.str;
          readOnly = true;
          default = controlGroupNames;
          description = ''
            Names of the groups this module creates, for
            `users.users.<name>.extraGroups`, so hosts don't repeat them as strings.
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
          }

          (lib.mkIf cfg.containerControl.enable {
            users.groups.${controlGroupNames.containerControl} = { };

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
                      && subject.isInGroup("${controlGroupNames.containerControl}")) {
                    return polkit.Result.YES;
                  }
                });
              '';
            };
          })

          (lib.mkIf cfg.hibernateDelayControl.enable {
            users.groups.${controlGroupNames.hibernateDelayControl} = { };

            systemd.services."sottomano-hibernate-delay@" = {
              description = "Set the suspend-then-hibernate delay to %i";
              serviceConfig = {
                Type = "oneshot";
                ExecStart = lib.concatStringsSep " " [
                  (lib.getExe pkgs.nushell)
                  "--no-config-file"
                  "${./write-hibernate-delay.nu}"
                  "%i"
                  hibernateDelayDropIn
                  "--shortest-delay ${toString cfg.hibernateDelayRange.shortestSeconds}sec"
                  "--longest-delay ${toString cfg.hibernateDelayRange.longestSeconds}sec"
                ];
                UMask = "0022";
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
                      && subject.isInGroup("${controlGroupNames.hibernateDelayControl}")) {
                    return polkit.Result.YES;
                  }
                });
              '';
            };
          })

          (lib.mkIf cfg.tailnetPolicyToken.enable {
            users.groups.${controlGroupNames.tailnetPolicyReaders} = { };

            systemd.services.${tailnetPolicyTokenServiceName} = {
              description = "Mint a read-only Tailscale policy token for sottomano";
              after = [ "network-online.target" ];
              wants = [ "network-online.target" ];
              serviceConfig = {
                Type = "oneshot";
                ExecStart = mintTailnetPolicyTokenCommand;
                LoadCredential = "${oauthClientCredentialName}:${cfg.tailnetPolicyToken.oauthClientEnvironmentFile}";
                DynamicUser = true;
                Group = controlGroupNames.tailnetPolicyReaders;
                RuntimeDirectory = tailnetPolicyRuntimeDirectory;
                RuntimeDirectoryMode = "0750";
                RuntimeDirectoryPreserve = true;
                UMask = "0027";
                Restart = "on-failure";
                RestartSec = "30s";
                TimeoutStartSec = "2min";
                CapabilityBoundingSet = "";
                LockPersonality = true;
                PrivateDevices = true;
                ProcSubset = "pid";
                ProtectClock = true;
                ProtectControlGroups = true;
                ProtectHostname = true;
                ProtectKernelLogs = true;
                ProtectKernelModules = true;
                ProtectKernelTunables = true;
                ProtectProc = "invisible";
                RestrictAddressFamilies = [
                  "AF_UNIX"
                  "AF_INET"
                  "AF_INET6"
                ];
                RestrictNamespaces = true;
                RestrictRealtime = true;
                SystemCallArchitectures = "native";
                SystemCallFilter = [
                  "@system-service"
                  "~@privileged @resources"
                ];
              };
            };

            systemd.timers.${tailnetPolicyTokenServiceName} = {
              wantedBy = cfg.tailnetPolicyToken.timerWantedBy;
              partOf = cfg.tailnetPolicyToken.timerWantedBy;
              timerConfig = {
                OnActiveSec = "0";
                OnUnitActiveSec = "30min";
              };
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
