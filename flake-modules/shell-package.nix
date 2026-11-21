{ config, ... }:
let
  supportedCompositors = config.sottomano.compositors;
in
{
  perSystem =
    { config, pkgs, ... }:
    let
      inherit (pkgs) lib;
      inherit (config.sottomano) cargoBuild desktopRuntime;
      inherit (cargoBuild) craneLib shellArgs;
      inherit (desktopRuntime) gsettingsRuntime;

      compositorCargoArgs =
        compositors:
        if lib.naturalSort compositors == lib.naturalSort supportedCompositors then
          shellArgs.cargoExtraArgs
        else
          "${shellArgs.cargoExtraArgs} --no-default-features --features ${
            lib.concatStringsSep "," (compositors ++ [ "libvirt-domains" ])
          }";

      buildSottomanoForCompositors =
        {
          compositors ? supportedCompositors,
        }:
        craneLib.buildPackage (
          shellArgs
          // {
            cargoArtifacts = cargoBuild.shellArtifacts;
            doCheck = false;
            cargoExtraArgs = compositorCargoArgs compositors;
            nativeBuildInputs = cargoBuild.commonArgs.nativeBuildInputs ++ [ pkgs.makeBinaryWrapper ];
            postInstall = ''
              mkdir -p $out/libexec
              mv $out/bin/sottomano $out/libexec/sottomano
              makeWrapper $out/libexec/sottomano $out/bin/sottomano \
                ${desktopRuntime.allocatorWrapperFlags} \
                --prefix PATH : ${gsettingsRuntime.binPath} \
                --prefix PATH : ${lib.makeBinPath desktopRuntime.runtimeDeps} \
                --prefix XDG_DATA_DIRS : ${gsettingsRuntime.schemaDir} \
                --prefix XDG_DATA_DIRS : ${desktopRuntime.soundThemeShare} \
                --prefix GIO_EXTRA_MODULES : ${gsettingsRuntime.gioModules}
            '';
            postFixup = ''
              patchelf --add-rpath ${lib.makeLibraryPath desktopRuntime.runtimeDlopenLibraries} $out/libexec/sottomano
            '';
            passthru = { inherit compositors; };
            meta = {
              mainProgram = "sottomano";
              platforms = lib.platforms.linux;
              license = lib.licenses.gpl3Plus;
            };
          }
        );

      sottomano = lib.makeOverridable buildSottomanoForCompositors { };
    in
    {
      packages = {
        default = sottomano;
        inherit sottomano;
      };

      checks = { inherit sottomano; };
    };
}
