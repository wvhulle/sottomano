# Home-manager module for vetrata, factored out of flake.nix to keep it
# readable. The caller passes the flake-level `self` (for the default package).
#
# Unlike the Quickshell Odin, vetrata has no umbra theme registry and no
# Quickshell wrapper: its config is a single static TOML file at
# ~/.config/vetrata.toml, and it runs its own niri window tracker (Turso), so this
# module neither imports umbra nor enables services.niri.windowTracker.
{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.vetrata;
  tomlFormat = pkgs.formats.toml { };

  # vetrata's niri layer-rules. The bar surface ("vetrata") wants square corners
  # and no drop shadow of its own; the two off-screen background-layer casters
  # ("vetrata-backdrop-<pos>") are the surfaces that bloom the gradient shadow
  # onto the screen edge so the bar's white text stays legible over light
  # wallpaper (see src/layer_surface.rs `backdrop_settings`). The casters only
  # exist when layout.shadow is set in vetrata.toml — the same toggle these rules
  # target, so the two stay in agreement.
  radius0 = {
    top-left = 0.0;
    top-right = 0.0;
    bottom-left = 0.0;
    bottom-right = 0.0;
  };
  gradientShadow = sign: {
    enable = true;
    inherit (cfg.niri.gradientShadow) softness spread color;
    offset = {
      x = 0;
      y = sign * cfg.niri.gradientShadow.offset;
    };
  };
  vetrataLayerRules = [
    {
      matches = [ { namespace = "^vetrata$"; } ];
      geometry-corner-radius = radius0;
      shadow.enable = false;
    }
    {
      matches = [ { namespace = "^vetrata-backdrop-top$"; } ];
      geometry-corner-radius = radius0;
      shadow = gradientShadow (-1);
    }
    {
      matches = [ { namespace = "^vetrata-backdrop-bottom$"; } ];
      geometry-corner-radius = radius0;
      shadow = gradientShadow 1;
    }
  ];
in
{
  options.services.vetrata = {
    enable = lib.mkEnableOption "vetrata bar";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default;
      defaultText = lib.literalExpression "vetrata.packages.\${system}.default";
      description = "The vetrata package to install.";
    };

    systemdUnit = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to create a systemd user service that starts vetrata with
        graphical-session.target. Disable to spawn vetrata from a
        compositor-specific hook instead.
      '';
    };

    settings = lib.mkOption {
      type = tomlFormat.type;
      default = { };
      example = lib.literalExpression ''
        {
          palette.bar_foreground = "#ffffff";
          layout = {
            outer_height_px = 28.0;
            # Cast the gradient legibility shadow (opens the off-screen casters
            # and activates the niri layer-rules below).
            shadow = true;
          };
          font.name = "Inter";
        }
      '';
      description = ''
        Contents of ~/.config/vetrata.toml, written verbatim. Maps to the
        `[palette]`, `[layout]`, `[font]` and `[notifications]` tables in
        vetrata (src/config/disk.rs). Every key has a default, so set only the
        ones you override; leave this empty to keep the built-in gruvbox-dark
        defaults. Set `layout.shadow = true` to enable the legibility shadow.
      '';
    };

    niri.gradientShadow = {
      softness = lib.mkOption {
        type = lib.types.ints.unsigned;
        default = 27;
        description = "Blur radius of the backdrop caster's drop shadow.";
      };
      spread = lib.mkOption {
        type = lib.types.ints.unsigned;
        default = 20;
        description = "How far the backdrop caster's drop shadow expands before blurring.";
      };
      color = lib.mkOption {
        type = lib.types.str;
        default = "#00000060";
        description = "Color of the backdrop caster's drop shadow (niri color string).";
      };
      offset = lib.mkOption {
        type = lib.types.ints.unsigned;
        default = 3;
        description = ''
          Vertical offset magnitude (px) of the backdrop caster's drop shadow,
          pushing the fade onto the screen. Applied toward the inner edge: the
          top caster uses -offset, the bottom caster +offset.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    # vetrata reads ~/.config/vetrata.toml directly (HOME/.config, not
    # XDG_CONFIG_HOME); xdg.configFile lands there. Only write it when the user
    # set something, so an empty config keeps vetrata's built-in defaults rather
    # than pinning an empty file.
    xdg.configFile."vetrata.toml" = lib.mkIf (cfg.settings != { }) {
      source = tomlFormat.generate "vetrata.toml" cfg.settings;
    };

    # Contribute vetrata's niri layer-rules. niri merges multi-matching rules in
    # order with later fields winning, so these MUST land after any catch-all
    # `namespace=".*"` rule; `lib.mkAfter` appends them. vetrata only runs under
    # niri, so this is unconditional.
    services.niri.settings.layer-rules = lib.mkAfter vetrataLayerRules;

    systemd.user.services.vetrata = lib.mkIf cfg.systemdUnit {
      Unit = {
        Description = "vetrata bar";
        PartOf = [ "graphical-session.target" ];
        After = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = lib.getExe cfg.package;
        Restart = "on-failure";
        RestartSec = 2;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
