{ inputs, ... }:
{
  imports = [ inputs.treefmt-nix.flakeModule ];

  perSystem =
    { config, ... }:
    {
      treefmt = {
        projectRootFile = "flake.nix";
        flakeCheck = false;
        settings.excludes = [
          "target/*"
          "recordings/*"
        ];
        programs = {
          nixfmt.enable = true;
          rustfmt = {
            enable = true;
            package = config.sottomano.cargoBuild.rustToolchain;
          };
        };
      };
    };
}
