{
  perSystem =
    { config, pkgs, ... }:
    let
      inherit (pkgs) lib;
      inherit (config.sottomano) cargoBuild;
      fontSubsetter = pkgs.python3.withPackages (pythonPackages: [
        pythonPackages.fonttools
        pythonPackages.brotli
      ]);
      latinUnicodes = "U+0000-00FF,U+0131,U+0152-0153,U+02C6,U+02DA,U+02DC,U+2000-206F,U+2190-21FF,U+2212,U+2215,U+2500-259F";

      showcaseRecorder = cargoBuild.craneLib.buildPackage (
        cargoBuild.showcaseArgs
        // cargoBuild.showcaseFonts
        // {
          cargoArtifacts = cargoBuild.showcaseArtifacts;
          doCheck = false;
        }
      );

      demoScriptDirectory = ../sottomano-tour-recordings/src/demo_script;
      demoNames =
        lib.concatMap
          (
            scriptFile:
            lib.concatMap (scriptPiece: if lib.isList scriptPiece then scriptPiece else [ ]) (
              builtins.split ''name: "([a-z0-9_]+)"'' (builtins.readFile (demoScriptDirectory + "/${scriptFile}"))
            )
          )
          (
            lib.attrNames (
              lib.filterAttrs (_: fileKind: fileKind == "regular") (builtins.readDir demoScriptDirectory)
            )
          );

      recordingRenderEnvironment = {
        inherit (cargoBuild.showcaseFonts) FONTCONFIG_FILE;
        nativeBuildInputs = [ pkgs.fontconfig ];
        LANG = "en_US.UTF-8";
        VK_ICD_FILENAMES = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";
        LD_LIBRARY_PATH = lib.makeLibraryPath [ pkgs.vulkan-loader ];
        LP_NUM_THREADS = "4";
      };

      renderedRecordings =
        derivationName: renderArguments:
        pkgs.runCommand derivationName recordingRenderEnvironment ''
          export HOME=$TMPDIR
          export XDG_CACHE_HOME=$TMPDIR/cache
          ${lib.getExe' showcaseRecorder "record"} render --output $out ${renderArguments}
        '';

      singleDemoRecordings = lib.listToAttrs (
        map (demoName: {
          name = "recording-${demoName}";
          value = renderedRecordings "sottomano-recording-${demoName}" "--jobs 1 ${demoName}";
        }) demoNames
      );

      showcaseRecordings = pkgs.runCommand "sottomano-tour-recordings" { } ''
        mkdir -p $out/recordings/stills
        ${lib.concatMapStrings (singleDemo: ''
          cp ${singleDemo}/recordings/*.webp $out/recordings/
          cp ${singleDemo}/recordings/stills/*.webp $out/recordings/stills/
        '') (lib.attrValues singleDemoRecordings)}
      '';

      documentationSite = pkgs.stdenvNoCC.mkDerivation {
        pname = "sottomano-documentation-site";
        version = "0";
        src = lib.fileset.toSource {
          root = ../.;
          fileset = lib.fileset.unions [
            ../book.toml
            ../docs
            ../README.md
            ../logo.svg
          ];
        };

        nativeBuildInputs = [
          pkgs.mdbook
          pkgs.mdbook-mermaid
          fontSubsetter
        ];
        nativeInstallCheckInputs = [ pkgs.lychee ];

        postPatch = ''
          mdbook-mermaid install .
          shopt -s globstar
          for markdown in README.md docs/**/*.md; do
            substituteInPlace "$markdown" \
              --replace-quiet https://codeberg.org/wvhulle/sottomano/raw/branch/main/logo.svg /logo.svg \
              --replace-quiet https://sottomano.work/ /
          done
          cd ${showcaseRecordings}
          for recording in recordings/**/*.webp; do
            content_hash=$(sha256sum "$recording" | head -c 12)
            recording_url="/$recording"
            sed -i "s#''${recording_url//./\\.}#&?v=$content_hash#g" "$OLDPWD"/README.md "$OLDPWD"/docs/**/*.md
          done
          cd "$OLDPWD"
        '';

        buildPhase = ''
          runHook preBuild
          mdbook build . --dest-dir site
          runHook postBuild
        '';

        installPhase = ''
          runHook preInstall
          mv site $out
          rm $out/theme/favicon.svg $out/theme/index.hbs
          mkdir -p $out/docs/theme/fonts
          pyftsubset "${pkgs.lilex}/share/fonts/truetype/Lilex[wght].ttf" \
            --unicodes="${latinUnicodes}" --layout-features='*' --flavor=woff2 \
            --output-file=$out/docs/theme/fonts/lilex-variable.woff2
          pyftsubset "${pkgs.lilex}/share/fonts/truetype/Lilex-Italic.ttf" \
            --unicodes="${latinUnicodes}" --layout-features='*' --flavor=woff2 \
            --output-file=$out/docs/theme/fonts/lilex-italic.woff2
          cp -r ${showcaseRecordings}/recordings $out/recordings
          cp logo.svg $out/logo.svg
          runHook postInstall
        '';

        dontFixup = true;

        doInstallCheck = true;
        SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
        installCheckPhase = ''
          runHook preInstallCheck
          if grep -rl --include='*.html' 'raw/branch/' $out; then
            echo "hotlinks to the Codeberg raw branches remain in the pages above"
            exit 1
          fi
          if [ -n "$(find $out -type l)" ]; then
            find $out -type l
            echo "static-web-server rejects symlinks that resolve outside the site root"
            exit 1
          fi
          lychee --no-progress --offline --include-fragments --root-dir $out "$out/**/*.html"
          runHook postInstallCheck
        '';
      };
    in
    {
      packages = {
        showcase-recordings = showcaseRecordings;
        documentation-site = documentationSite;
      }
      // singleDemoRecordings;

      apps.documentation-preview.program = lib.getExe (
        pkgs.writeShellApplication {
          name = "documentation-preview";
          runtimeInputs = [ pkgs.static-web-server ];
          text = ''
            port="''${1:-8000}"
            echo "serving the documentation site on http://localhost:$port"
            exec static-web-server --port "$port" --root ${documentationSite} --index-files index.html
          '';
        }
      );

      checks.documentation-site = documentationSite;
    };
}
