use std::path::Path;

use serde::Deserialize;

use crate::{
    desktop_windows_apps::{
        nix_flake_pin::flake_pin,
        nix_package_facts::{
            Definition, Generation, NixpkgsEntry, NixpkgsListing, PackageHomepage, PackageLicense,
        },
        nix_store_provenance::{StorePackage, below_derivation},
    },
    system_protocol_clients::shell_command_output,
};

const NIXPKGS_META: &str = "p: {
  storePath = p.outPath;
  inherit (p) version;
  position = p.meta.position or null;
  homepage = let h = p.meta.homepage or null; in if builtins.isList h then builtins.head h else h;
  license = p.meta.license.spdxId or null;
}";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NixosVersion {
    nixpkgs_revision: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvaluatedMeta {
    store_path: String,
    version: String,
    position: Option<String>,
    homepage: Option<String>,
    license: Option<String>,
}

pub(super) async fn nixpkgs_listing(
    package: &StorePackage,
    root: &str,
    nixos_flake: &Path,
) -> NixpkgsListing {
    nixpkgs_entry(package, root, nixos_flake)
        .await
        .map_or(NixpkgsListing::Unlisted, NixpkgsListing::Listed)
}

async fn nixpkgs_entry(
    package: &StorePackage,
    root: &str,
    nixos_flake: &Path,
) -> Option<NixpkgsEntry> {
    let raw = shell_command_output::output("nixos-version", &["--json"])
        .await
        .inspect_err(|command_failure| {
            tracing::warn!("nix facts: nixos-version: {command_failure}");
        })
        .ok()?;
    let system: NixosVersion = serde_json::from_str(&raw)
        .inspect_err(|parse_failure| {
            tracing::warn!("nix facts: nixos-version output: {parse_failure}");
        })
        .ok()?;
    let revision = system.nixpkgs_revision;
    let installable = format!("github:NixOS/nixpkgs/{revision}#{}", package.name);
    let raw = shell_command_output::output(
        "nix",
        &["eval", "--json", &installable, "--apply", NIXPKGS_META],
    )
    .await
    .inspect_err(|command_failure| {
        tracing::warn!("nix facts: eval {installable}: {command_failure}");
    })
    .ok()?;
    let meta: EvaluatedMeta = serde_json::from_str(&raw)
        .inspect_err(|parse_failure| {
            tracing::warn!("nix facts: meta of {installable}: {parse_failure}");
        })
        .ok()?;
    let pin = flake_pin(&package.name, nixos_flake, &revision, &meta.version).await;
    let generation = if meta.store_path == root {
        Generation::Current
    } else {
        Generation::Rebuilt {
            version: meta.version,
        }
    };
    Some(NixpkgsEntry {
        definition: definition(&meta.position?, &revision)?,
        homepage: meta
            .homepage
            .map_or(PackageHomepage::Unlisted, PackageHomepage::Listed),
        license: meta
            .license
            .map_or(PackageLicense::Unidentified, PackageLicense::Spdx),
        generation,
        pin,
    })
}

fn definition(position: &str, revision: &str) -> Option<Definition> {
    let (file, line) = position.rsplit_once(':')?;
    let relative = below_derivation(file)?;
    Some(Definition {
        file: format!("{relative}:{line}"),
        url: format!("https://github.com/NixOS/nixpkgs/blob/{revision}/{relative}#L{line}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_maps_to_a_github_blob_line() {
        assert_eq!(
            definition(
                "/nix/store/fn45aans2f66gvpm8i51ddbi4y4brrhm-source/pkgs/by-name/ki/kitty/package.nix:345",
                "39ad350a0602fa0a58a544344e3e9187526ea45c"
            ),
            Some(Definition {
                file: "pkgs/by-name/ki/kitty/package.nix:345".to_owned(),
                url: "https://github.com/NixOS/nixpkgs/blob/39ad350a0602fa0a58a544344e3e9187526ea45c/pkgs/by-name/ki/kitty/package.nix#L345".to_owned(),
            })
        );
        assert_eq!(definition("/home/me/package.nix:3", "rev"), None);
    }
}
