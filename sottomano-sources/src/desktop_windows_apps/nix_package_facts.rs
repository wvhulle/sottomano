use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    time::{Duration, SystemTime},
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt, stream};
use serde::Deserialize;

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    delivery_cadence_drivers::PeriodicPolling,
    desktop_windows_apps::{
        nix_store_provenance::{Provenance, StorePackage, below_derivation},
        process_tree_inspection::{Watched, launch},
    },
    stream_operators::StreamOperators,
    system_protocol_clients::shell_command_output,
};

const NIXPKGS_META: &str = "p: {
  storePath = p.outPath;
  inherit (p) version;
  position = p.meta.position or null;
  homepage = let h = p.meta.homepage or null; in if builtins.isList h then builtins.head h else h;
  description = p.meta.description or null;
  license = p.meta.license.spdxId or null;
}";

const UPSTREAM_VERSION: &str = "p: { storePath = p.outPath; inherit (p) version; }";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageWatch {
    pub watched: Watched,
    pub upstream_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    OutsideStore,
    Command(shell_command_output::Error),
    Malformed,
    Interrupted,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::OutsideStore => f.write_str("binary is not in the nix store"),
            Error::Command(error) => error.fmt(f),
            Error::Malformed => f.write_str("nix returned unreadable output"),
            Error::Interrupted => Panicked.fmt(f),
        }
    }
}

impl From<shell_command_output::Error> for Error {
    fn from(error: shell_command_output::Error) -> Self {
        Error::Command(error)
    }
}

impl From<Panicked> for Error {
    fn from(_: Panicked) -> Self {
        Error::Interrupted
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureFacts {
    pub closure_size: ByteSize,
    pub path_count: usize,
    pub own_size: ByteSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Signed { by: Vec<String> },
    BuiltLocally,
    Unsigned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub file: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NixpkgsEntry {
    pub definition: Definition,
    pub homepage: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub generation: Generation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Generation {
    Current,
    Rebuilt { version: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upstream {
    pub branch: String,
    pub state: UpstreamState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamState {
    UpToDate,
    Newer { version: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NixPackageFacts {
    pub closure: ClosureFacts,
    pub origin: Origin,
    pub registered: SystemTime,
    pub nixpkgs: Option<NixpkgsEntry>,
    pub upstream: Option<Upstream>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathInfo {
    closure_size: u64,
    nar_size: u64,
    registration_time: u64,
    #[serde(default)]
    signatures: Vec<String>,
    #[serde(default)]
    ultimate: bool,
}

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
    description: Option<String>,
    license: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvaluatedVersion {
    store_path: String,
    version: String,
}

impl SnapshotSource for NixPackageFacts {
    type Error = Error;
    type StreamInputs = PackageWatch;

    const NAME: &'static str = "nix store";

    fn delivery_stream(inputs: PackageWatch) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("nix")).while_held(
            move || {
                PeriodicPolling::every(Duration::from_secs(600))
                    .poll_deliveries_after_setup(
                        InstalledPackage::read(inputs.clone()),
                        Self::poll_upstream,
                    )
                    .map(absent_outside_store)
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

#[derive(Debug, Clone)]
pub struct InstalledPackage {
    package: StorePackage,
    root: String,
    upstream_branch: String,
    facts: NixPackageFacts,
}

impl InstalledPackage {
    async fn read(key: PackageWatch) -> Result<Self, Error> {
        let watched = key.watched;
        let package = blocking_poll(move || hovered_package(&watched)).await?;
        let root = package.root.to_string_lossy().into_owned();
        let info = path_info(&root).await?;
        let path_count = shell_command_output::output("nix", &["path-info", "-r", &root])
            .await?
            .lines()
            .count();
        let nixpkgs = nixpkgs_entry(&package, &root).await;
        Ok(Self {
            facts: NixPackageFacts {
                closure: ClosureFacts {
                    closure_size: ByteSize::b(info.closure_size),
                    path_count,
                    own_size: ByteSize::b(info.nar_size),
                },
                origin: origin(&info),
                registered: SystemTime::UNIX_EPOCH + Duration::from_secs(info.registration_time),
                nixpkgs,
                upstream: None,
            },
            package,
            root,
            upstream_branch: key.upstream_branch,
        })
    }
}

impl NixPackageFacts {
    async fn poll_upstream(installed: InstalledPackage) -> Result<Self, Error> {
        let state = upstream_state(
            &installed.package,
            &installed.root,
            &installed.upstream_branch,
        )
        .await;
        Ok(Self {
            upstream: state.map(|state| Upstream {
                branch: installed.upstream_branch,
                state,
            }),
            ..installed.facts
        })
    }
}

fn absent_outside_store(
    delivery: SnapshotDelivery<NixPackageFacts>,
) -> SnapshotDelivery<NixPackageFacts> {
    match delivery {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch: Err(Error::OutsideStore),
            ..
        } => SnapshotDelivery::SourceUnavailable,
        delivered => delivered,
    }
}

fn hovered_package(watched: &Watched) -> Result<StorePackage, Error> {
    let launched = launch(watched.pid).ok_or(Error::OutsideStore)?;
    let binary = launched.binary.as_deref().ok_or(Error::OutsideStore)?;
    Provenance::of(binary, &launched.arguments, watched.app_id.as_deref())
        .package
        .ok_or(Error::OutsideStore)
}

async fn path_info(root: &str) -> Result<PathInfo, Error> {
    let raw = shell_command_output::output(
        "nix",
        &["path-info", "--json", "--json-format", "1", "-S", root],
    )
    .await?;
    let mut infos: HashMap<String, PathInfo> =
        serde_json::from_str(&raw).map_err(|_| Error::Malformed)?;
    infos.remove(root).ok_or(Error::Malformed)
}

fn origin(info: &PathInfo) -> Origin {
    if info.ultimate {
        Origin::BuiltLocally
    } else if info.signatures.is_empty() {
        Origin::Unsigned
    } else {
        Origin::Signed {
            by: info
                .signatures
                .iter()
                .filter_map(|signature| signature.split_once(':'))
                .map(|(signer, _)| signer.to_owned())
                .collect(),
        }
    }
}

async fn nixpkgs_entry(package: &StorePackage, root: &str) -> Option<NixpkgsEntry> {
    let raw = shell_command_output::output("nixos-version", &["--json"])
        .await
        .ok()?;
    let system: NixosVersion = serde_json::from_str(&raw).ok()?;
    let revision = system.nixpkgs_revision;
    let installable = format!("github:NixOS/nixpkgs/{revision}#{}", package.name);
    let raw = shell_command_output::output(
        "nix",
        &["eval", "--json", &installable, "--apply", NIXPKGS_META],
    )
    .await
    .ok()?;
    let meta: EvaluatedMeta = serde_json::from_str(&raw).ok()?;
    let generation = if meta.store_path == root {
        Generation::Current
    } else {
        Generation::Rebuilt {
            version: meta.version,
        }
    };
    Some(NixpkgsEntry {
        definition: definition(&meta.position?, &revision)?,
        homepage: meta.homepage,
        description: meta.description,
        license: meta.license,
        generation,
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

async fn upstream_state(package: &StorePackage, root: &str, branch: &str) -> Option<UpstreamState> {
    let installable = format!("github:NixOS/nixpkgs/{branch}#{}", package.name);
    let raw = shell_command_output::output(
        "nix",
        &["eval", "--json", &installable, "--apply", UPSTREAM_VERSION],
    )
    .await
    .ok()?;
    let evaluated: EvaluatedVersion = serde_json::from_str(&raw).ok()?;
    let same_build = evaluated.store_path == root;
    let same_version = package.version.as_deref() == Some(evaluated.version.as_str());
    Some(if same_build || same_version {
        UpstreamState::UpToDate
    } else {
        UpstreamState::Newer {
            version: evaluated.version,
        }
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

    #[test]
    fn path_info_json_yields_sizes_and_signers() {
        let raw = r#"{"/nix/store/702d-kitty-0.49.2":{"ca":null,"closureSize":620696008,"deriver":"/nix/store/x.drv","narHash":"sha256-x","narSize":81295480,"references":[],"registrationTime":1791565317,"signatures":["cache.nixos.org-1:zqpZ"],"ultimate":false,"version":1}}"#;
        let mut infos: HashMap<String, PathInfo> = serde_json::from_str(raw).unwrap();
        let info = infos.remove("/nix/store/702d-kitty-0.49.2").unwrap();
        assert_eq!(info.closure_size, 620_696_008);
        assert_eq!(info.nar_size, 81_295_480);
        assert_eq!(
            origin(&info),
            Origin::Signed {
                by: vec!["cache.nixos.org-1".to_owned()]
            }
        );
    }
}
