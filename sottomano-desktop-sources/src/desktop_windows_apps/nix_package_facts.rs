use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    path::PathBuf,
    time::{Duration, SystemTime},
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt, stream};
use serde::Deserialize;

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    blocking_background_work::{Panicked, blocking_poll},
    delivery_cadence_drivers::PeriodicPolling,
    desktop_windows_apps::{
        nix_flake_pin::{FlakePin, evaluate_version},
        nix_store_provenance::{LaunchPackaging, PackageVersion, Provenance, StorePackage},
        process_tree_inspection::{LaunchBinary, Watched, launch},
    },
    system_protocol_clients::shell_command_output,
};

mod nixpkgs_meta_entry;

use nixpkgs_meta_entry::nixpkgs_listing;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageWatch {
    pub watched: Watched,
    pub upstream_branch: String,
    pub nixos_flake: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    OutsideStore,
    Command(shell_command_output::Error),
    Malformed,
    Interrupted,
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::OutsideStore => formatter.write_str("binary is not in the nix store"),
            Error::Command(error) => error.fmt(formatter),
            Error::Malformed => formatter.write_str("nix returned unreadable output"),
            Error::Interrupted => Panicked.fmt(formatter),
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
pub enum PackageHomepage {
    Listed(String),
    Unlisted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLicense {
    Spdx(String),
    Unidentified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NixpkgsEntry {
    pub definition: Definition,
    pub homepage: PackageHomepage,
    pub license: PackageLicense,
    pub generation: Generation,
    pub pin: FlakePin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NixpkgsListing {
    Listed(NixpkgsEntry),
    Unlisted,
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
    Unevaluated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NixPackageFacts {
    pub running: StorePackage,
    pub closure: ClosureFacts,
    pub origin: Origin,
    pub registered: SystemTime,
    pub nixpkgs: NixpkgsListing,
    pub upstream: Upstream,
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

impl SnapshotSource for NixPackageFacts {
    type Error = Error;
    type StreamInputs = PackageWatch;

    const NAME: &'static str = "Nix";

    fn delivery_stream(inputs: PackageWatch) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::program_availability("nix")).while_available(
            move || {
                PeriodicPolling::every(Duration::from_secs(600))
                    .poll_deliveries_after_setup(
                        InstalledPackage::read(inputs.clone()),
                        Self::poll_upstream,
                    )
                    .map(absent_outside_store)
            },
            || SnapshotDelivery::SourceUnavailable,
        )
    }
}

#[derive(Debug, Clone)]
pub struct InstalledPackage {
    package: StorePackage,
    root: String,
    upstream_branch: String,
    closure: ClosureFacts,
    origin: Origin,
    registered: SystemTime,
    nixpkgs: NixpkgsListing,
}

impl InstalledPackage {
    async fn read(key: PackageWatch) -> Result<Self, Error> {
        let watched = key.watched;
        let package = blocking_poll(move || hovered_package(&watched)).await?;
        let root = package
            .root
            .to_str()
            .expect("nix store path names are ascii")
            .to_owned();
        let info = path_info(&root).await?;
        let path_count = shell_command_output::output("nix", &["path-info", "-r", &root])
            .await?
            .lines()
            .count();
        let nixpkgs = nixpkgs_listing(&package, &root, &key.nixos_flake).await;
        Ok(Self {
            closure: ClosureFacts {
                closure_size: ByteSize::b(info.closure_size),
                path_count,
                own_size: ByteSize::b(info.nar_size),
            },
            origin: origin(&info),
            registered: SystemTime::UNIX_EPOCH + Duration::from_secs(info.registration_time),
            nixpkgs,
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
            running: installed.package,
            closure: installed.closure,
            origin: installed.origin,
            registered: installed.registered,
            nixpkgs: installed.nixpkgs,
            upstream: Upstream {
                branch: installed.upstream_branch,
                state,
            },
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
    let LaunchBinary::Resolved(binary) = &launched.binary else {
        return Err(Error::OutsideStore);
    };
    match Provenance::trace_launch(binary, &launched.arguments, &watched.app_id).packaging {
        LaunchPackaging::Packaged { package, .. } => Ok(package),
        LaunchPackaging::Unpackaged => Err(Error::OutsideStore),
    }
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

async fn upstream_state(package: &StorePackage, root: &str, branch: &str) -> UpstreamState {
    let installable = format!("github:NixOS/nixpkgs/{branch}#{}", package.name);
    let Some(evaluated) = evaluate_version(&installable).await else {
        return UpstreamState::Unevaluated;
    };
    let same_build = evaluated.store_path == root;
    let same_version = package.version == PackageVersion::Versioned(evaluated.version.clone());
    if same_build || same_version {
        UpstreamState::UpToDate
    } else {
        UpstreamState::Newer {
            version: evaluated.version,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
