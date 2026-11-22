{
  config,
  inputs,
  moduleWithSystem,
  ...
}:
let
  supportedCompositors = config.sottomano.compositors;
in
{
  flake.homeModules.default = {
    imports = [
      inputs.niri-hm.homeModules.default
      (moduleWithSystem (
        { config }:
        let
          flakePackage = config.packages.default;
        in
        {
          config,
          options,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.services.sottomano;
          themeCfg = config.services.umbra or null;
          tomlFormat = pkgs.formats.toml { };
          qmpSocketQemuOptions = "-qmp unix:qmp.sock,server=on,wait=off";
          compositorsNarrowed = lib.naturalSort cfg.compositors != lib.naturalSort supportedCompositors;
          packageNarrowable = cfg.package ? compositors && cfg.package ? override;
          packageForCompositors =
            if compositorsNarrowed && packageNarrowable then
              cfg.package.override { inherit (cfg) compositors; }
            else
              cfg.package;

          modeToPalette = base16Palette: {
            background = base16Palette.base00;
            background_weak = base16Palette.base02;
            background_strong = base16Palette.base03;
            text = base16Palette.base05;
            secondary = base16Palette.base04;
            primary = base16Palette.base0C;
            success = base16Palette.base0B;
            warning = base16Palette.base0A;
            danger = base16Palette.base08;
          };

          variantToml =
            modePalette:
            tomlFormat.generate "sottomano.toml" (
              lib.recursiveUpdate cfg.settings {
                theme.palette = modeToPalette modePalette // (cfg.settings.theme.palette or { });
              }
            );
        in
        {
          imports =
            lib.mapAttrsToList
              (
                removed_option_name: secrets_table_key:
                lib.mkRemovedOptionModule [ "services" "sottomano" "secretFiles" removed_option_name ] ''
                  Set services.sottomano.settings.secrets.${secrets_table_key} to the secret's path instead;
                  sottomano reads secret paths from the [secrets] table of sottomano.toml.
                ''
              )
              {
                "openrouter-management-key" = "openrouter_management_key";
                "claude-code-oauth" = "claude_code_oauth";
                "gh-auth-token" = "gh_auth_token";
                "unsplash-access-key" = "unsplash_access_key";
                "tailscale-api-token" = "tailscale_api_token";
                "caldav-password" = "caldav_password";
              };

          options.services.sottomano = {
            enable = lib.mkEnableOption "sottomano bar";

            package = lib.mkOption {
              type = lib.types.package;
              default = flakePackage;
              defaultText = lib.literalExpression "sottomano.packages.\${system}.default";
              description = "The sottomano package to install.";
            };

            compositors = lib.mkOption {
              type = lib.types.listOf (lib.types.enum supportedCompositors);
              default = supportedCompositors;
              example = [ "niri" ];
              description = ''
                Compositors whose support is compiled into the package. Sottomano
                finds the running one from its socket variable at startup, so listing
                only the compositor you use makes a smaller build without changing
                behaviour.
              '';
            };

            systemdUnit = lib.mkOption {
              type = lib.types.bool;
              default = true;
              description = ''
                Whether to create a systemd user service that starts sottomano with
                graphical-session.target. Disable to spawn sottomano from a
                compositor-specific hook instead.
              '';
            };

            settings = lib.mkOption {
              inherit (tomlFormat) type;
              default = { };
              description = ''
                Contents of ~/.config/sottomano.toml, written verbatim. The running bar
                watches the file and applies changes in place. `sottomano
                default-config` prints every key with its default value. Secret files
                go in `settings.secrets` as paths (for example
                `settings.secrets.caldav_password = config.age.secrets.caldav.path`), so
                no secret enters the Nix store; `sottomano secrets` lists the keys.
              '';
            };

            umbra.enable = lib.mkOption {
              type = lib.types.bool;
              default = options.services ? umbra;
              defaultText = lib.literalExpression "options.services ? umbra";
              description = ''
                Runtime dark/light palette switching via the umbra app registry.
                Defaults to on when the umbra home-manager module is imported.
                Requires services.umbra (from umbra.homeModules) in the same
                home-manager config: registers services.umbra.apps.sottomano with
                dark/light palette variants derived from services.umbra.palettes.*. On
                a mode switch umbra re-points ~/.config/sottomano.toml and the running
                bar reloads it in place. Set false for a standalone
                install with no umbra — sottomano then falls back to the static
                palette in settings.
              '';
            };

            windowAgents.kitty.enable = lib.mkEnableOption ''
              kitty's socket-only remote control (at `mkDefault` priority, so an
              existing kitty config wins), which the bar's kitty window agent uses to
              tell apart kitty OS windows sharing one `--single-instance` process
            '';

            virtualMachines.qmpSocket.enable = lib.mkOption {
              type = lib.types.bool;
              default = true;
              description = ''
                Set QEMU_OPTS (at `mkDefault` priority, so an existing value wins) to
                give every NixOS `run-*-vm` script started in this session a QMP
                socket in its own temporary directory. The virtual machines group
                reads it to show whether a VM is paused and to power it off.
              '';
            };

          };

          config = lib.mkIf cfg.enable {
            home.sessionVariables = lib.mkIf cfg.virtualMachines.qmpSocket.enable {
              QEMU_OPTS = lib.mkDefault qmpSocketQemuOptions;
            };

            systemd.user.sessionVariables = lib.mkIf cfg.virtualMachines.qmpSocket.enable {
              QEMU_OPTS = lib.mkDefault qmpSocketQemuOptions;
            };

            programs.kitty.settings = lib.mkIf cfg.windowAgents.kitty.enable {
              allow_remote_control = lib.mkDefault "socket-only";
              listen_on = lib.mkDefault "unix:$XDG_RUNTIME_DIR/kitty.sock";
            };

            assertions = [
              {
                assertion = cfg.compositors != [ ];
                message = "services.sottomano.compositors needs at least one compositor.";
              }
              {
                assertion = !compositorsNarrowed || packageNarrowable;
                message = ''
                  services.sottomano.compositors can only narrow the package from the
                  sottomano flake; the configured services.sottomano.package cannot be
                  rebuilt with fewer compositors.
                '';
              }
            ]
            ++ lib.optionals cfg.umbra.enable [
              {
                assertion = themeCfg != null && themeCfg ? palettes;
                message = ''
                  services.sottomano.umbra.enable requires services.umbra (provided by
                  umbra.homeModules.default/schema). Import it in the same home-manager
                  config, or set services.sottomano.umbra.enable = false.
                '';
              }
            ];

            home.packages = [ packageForCompositors ];

            xdg.configFile = {
              "sottomano.toml" = lib.mkIf (!cfg.umbra.enable && cfg.settings != { }) {
                source = tomlFormat.generate "sottomano.toml" cfg.settings;
              };

              "systemd/user/niri.service.d/sottomano-priority.conf" =
                lib.mkIf (cfg.systemdUnit && config.services.niri.enable)
                  {
                    text = ''
                      [Service]
                      CPUWeight=1000
                    '';
                  };
            };

            services.umbra.apps.sottomano = lib.mkIf cfg.umbra.enable {
              target = "sottomano.toml";
              dark.source = variantToml themeCfg.palettes.dark;
              light.source = variantToml themeCfg.palettes.light;
            };

            services.niri.settings.includes = lib.mkIf config.services.niri.enable [
              {
                path = "${config.xdg.configHome}/sottomano/niri.kdl";
                optional = true;
              }
            ];

            wayland.windowManager.sway.extraConfig = lib.mkIf config.wayland.windowManager.sway.enable ''
              include ${config.xdg.configHome}/sottomano/sway.conf
            '';

            wayland.windowManager.hyprland.extraConfig =
              let
                hyprlandConfig = config.wayland.windowManager.hyprland;
              in
              lib.mkIf (hyprlandConfig.enable && hyprlandConfig.configType == "lua") (
                lib.mkAfter ''
                  local sottomano_rules_path = ${builtins.toJSON "${config.xdg.configHome}/sottomano/hyprland.lua"}
                  local sottomano_rules_file = io.open(sottomano_rules_path)
                  if sottomano_rules_file then
                    sottomano_rules_file:close()
                    dofile(sottomano_rules_path)
                  end
                ''
              );

            systemd.user.services.sottomano = lib.mkIf cfg.systemdUnit {
              Unit = {
                Description = "sottomano bar";
                PartOf = [ "graphical-session.target" ];
                After = [ "graphical-session.target" ];
              };
              Service = {
                ExecStart = lib.getExe packageForCompositors;
                Restart = "on-failure";
                RestartSec = 2;
                Slice = "session.slice";
                CPUWeight = 500;
                MemoryMin = lib.mkDefault "300M";
              };
              Install.WantedBy = [ "graphical-session.target" ];
            };
          };
        }
      ))
    ];
  };
}
