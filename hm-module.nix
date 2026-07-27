# Home-manager module for vetrata, factored out of flake.nix to keep it
# readable. The caller passes the flake-level `self` (for the default package).
#
# Unlike the Quickshell Odin, vetrata has no Quickshell wrapper and runs its own
# niri window tracker (Turso), so it never enables services.niri.windowTracker.
# It reads a single static TOML at ~/.config/vetrata.toml, resolved once at
# startup (no hot reload). When `umbra.enable` is on (the default) it registers
# with the umbra app registry for runtime dark/light palette switching — it does
# NOT import the umbra module itself; the desktop layer supplies services.umbra.
{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.vetrata;
  themeCfg = config.services.umbra or null;
  tomlFormat = pkgs.formats.toml { };

  # The bar surface is transparent (bar_background left unset) over the wallpaper
  # with a legibility shadow, so its text must stay light in EVERY mode. base16
  # slots invert between light and dark schemes (base05 is light in a dark
  # scheme, dark in a light one), so no single slot reads "light" in both modes.
  # Hence the foreground family (text/inactive/dim) is pinned to the DARK
  # scheme's light slots for both variants; only the semantic accents follow the
  # active base16 mode. Names describe the UI role, not syntax.
  foregroundPalette = p: {
    bar_foreground = p.base05; # main text (light)
    inactive = p.base04; # muted text (light)
    dim = p.base03; # faint text
  };
  accentPalette = p: {
    urgent = p.base08; # red
    warning = p.base0A; # yellow
    focus = p.base0C; # aqua
  };

  # One per-mode TOML file: the user's `settings` (layout/font/notifications) as
  # the base, then the fixed light foreground + the mode's accents merged in.
  # Any palette key the user set explicitly still wins.
  variantToml =
    modePalette:
    tomlFormat.generate "vetrata.toml" (
      lib.recursiveUpdate cfg.settings {
        palette =
          foregroundPalette themeCfg.palettes.dark
          // accentPalette modePalette
          // (cfg.settings.palette or { });
      }
    );

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
    # Each toast has its own tightly-sized overlay surface
    # (namespace "vetrata-notifications", src/notifications/stack.rs), so niri can
    # hug a drop shadow straight to the card — no off-screen caster like the bar.
    # Corners match the card's default `layout.corner_radius_px` (src/config/disk.rs).
    {
      matches = [ { namespace = "^vetrata-notifications$"; } ];
      geometry-corner-radius = {
        top-left = 8.0;
        top-right = 8.0;
        bottom-left = 8.0;
        bottom-right = 8.0;
      };
      shadow = {
        enable = true;
        softness = 30;
        spread = 10;
        color = "#00000070";
      };
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

    umbra.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Runtime dark/light palette switching via the umbra app registry.
        Requires services.umbra (from umbra.homeModules) in the same
        home-manager config: registers services.umbra.apps.vetrata with
        dark/light palette variants derived from services.umbra.palettes.* and
        restarts vetrata.service on mode switch. Set false for a standalone
        install with no umbra — vetrata then falls back to the static
        settings.palette written to ~/.config/vetrata.toml.
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
    assertions = lib.optionals cfg.umbra.enable [
      {
        assertion = themeCfg != null && themeCfg ? palettes;
        message = ''
          services.vetrata.umbra.enable requires services.umbra (provided by
          umbra.homeModules.default/schema). Import it in the same home-manager
          config, or set services.vetrata.umbra.enable = false.
        '';
      }
    ];

    home.packages = [ cfg.package ];

    # vetrata reads ~/.config/vetrata.toml directly (HOME/.config, not
    # XDG_CONFIG_HOME); xdg.configFile lands there. Only write it when the user
    # set something, so an empty config keeps vetrata's built-in defaults rather
    # than pinning an empty file. When umbra is on it owns this path at runtime
    # (ln -sf between the dark-/light- variants it declares), so we must NOT
    # also declare it here — the two would collide.
    xdg.configFile."vetrata.toml" = lib.mkIf (!cfg.umbra.enable && cfg.settings != { }) {
      source = tomlFormat.generate "vetrata.toml" cfg.settings;
    };

    # Register with the umbra app registry: umbra writes the dark-/light-vetrata
    # variant files and swaps ~/.config/vetrata.toml between them on mode switch.
    # vetrata reads its config only at startup and has no IPC, so the swap must
    # restart it — reload.restartUnits makes umbra do exactly that (a systemd
    # path unit can't see the `ln -sf`, hence this rather than a path watch).
    services.umbra.apps.vetrata = lib.mkIf cfg.umbra.enable {
      target = "vetrata.toml";
      dark.source = variantToml themeCfg.palettes.dark;
      light.source = variantToml themeCfg.palettes.light;
      reload.restartUnits = [ "vetrata.service" ];
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

    # Restart vetrata when its config file is edited in place. This complements
    # umbra's reload.restartUnits (which handles the dark/light symlink swap):
    # a path unit fires on writes to the file, not on `ln -sf`, so it covers
    # direct edits of a real ~/.config/vetrata.toml. A path unit only *starts*
    # its unit, and starting an already-running service is a no-op, so it
    # triggers a oneshot that restarts instead.
    systemd.user.paths.vetrata = lib.mkIf cfg.systemdUnit {
      Unit = {
        Description = "Watch vetrata config and restart the bar on change";
        PartOf = [ "graphical-session.target" ];
      };
      Path = {
        PathModified = "%h/.config/vetrata.toml";
        Unit = "vetrata-reload.service";
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };

    systemd.user.services.vetrata-reload = lib.mkIf cfg.systemdUnit {
      Unit = {
        Description = "Restart vetrata after a config change";
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        Type = "oneshot";
        ExecStart = "${pkgs.systemd}/bin/systemctl --user restart vetrata.service";
      };
    };
  };
}
