use std::{path::PathBuf, time::Duration};

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    desktop_windows_apps::{
        nix_flake_pin::FlakePin,
        nix_package_facts::{
            ClosureFacts, Definition, Generation, NixPackageFacts, NixpkgsEntry, NixpkgsListing,
            Origin, PackageHomepage, PackageLicense, Upstream, UpstreamState,
        },
        nix_store_provenance::{
            LaunchPackaging, PackageRuntime, PackageVersion, ProfileProvider, Provenance,
            StorePackage,
        },
        process_tree_inspection::{CgroupScope, Inspection, Launch, LaunchBinary, Process},
    },
};
use sottomano_widgets::{
    hover_cards::HoverTarget,
    output_windows::{OutputWindowsMessage, WindowTarget, window_list::WindowListMessage},
};
use uom::si::{f64::Ratio, ratio::percent};

use crate::fixtures::{
    desktop_session::{TERMINAL_PID, TERMINAL_WINDOW},
    showcase_epoch,
};

pub fn on_hover(target: &HoverTarget) -> Vec<OutputWindowsMessage> {
    match target {
        HoverTarget::Window(WindowTarget::Inspect(TERMINAL_WINDOW)) => vec![
            OutputWindowsMessage::WindowList(WindowListMessage::ProcessTreeScanned(
                SnapshotDelivery::fixture_delivered(
                    Ok(terminal_processes()),
                    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(
                        1,
                    ))),
                ),
            )),
            OutputWindowsMessage::WindowList(WindowListMessage::ProcessInspected(Box::new(
                SnapshotDelivery::single_read(Ok(terminal_inspection())),
            ))),
            OutputWindowsMessage::WindowList(WindowListMessage::PackageFactsRead(Box::new(
                SnapshotDelivery::single_read(Ok(terminal_package_facts())),
            ))),
        ],
        _ => Vec::new(),
    }
}

fn process(pid: i32, parent: i32, name: &str, mebibytes: u64, children: Vec<Process>) -> Process {
    Process {
        pid,
        parent,
        name: name.to_owned(),
        memory: ByteSize::mib(mebibytes),
        cpu: Ratio::new::<percent>(0.4),
        threads: 4,
        uptime: Duration::from_hours(3),
        children,
    }
}

fn terminal_processes() -> Process {
    process(
        TERMINAL_PID,
        1,
        "foot",
        42,
        vec![process(
            TERMINAL_PID + 10,
            TERMINAL_PID,
            "nu",
            64,
            vec![process(
                TERMINAL_PID + 11,
                TERMINAL_PID + 10,
                "cargo",
                380,
                vec![],
            )],
        )],
    )
}

fn foot_store_package() -> StorePackage {
    StorePackage {
        root: PathBuf::from("/nix/store/7rj0kcq2w9j8foot-1.23.1"),
        name: "foot".to_owned(),
        version: PackageVersion::Versioned("1.23.1".to_owned()),
    }
}

fn terminal_inspection() -> Inspection {
    let foot = foot_store_package();
    Inspection {
        pid: TERMINAL_PID,
        launch: Launch {
            binary: LaunchBinary::Resolved(foot.root.join("bin/foot")),
            arguments: Vec::new(),
            scope: CgroupScope::Unit("niri.service".to_owned()),
        },
        provenance: Provenance {
            packaging: LaunchPackaging::Packaged {
                package: foot,
                runtime: PackageRuntime::WithinPackage,
            },
            referenced: Vec::new(),
            profile: ProfileProvider::Provided("home-manager-path".to_owned()),
        },
    }
}

fn terminal_package_facts() -> NixPackageFacts {
    NixPackageFacts {
        running: foot_store_package(),
        closure: ClosureFacts {
            closure_size: ByteSize::mib(96),
            path_count: 41,
            own_size: ByteSize::kib(1_380),
        },
        origin: Origin::Signed {
            by: vec!["cache.nixos.org-1".to_owned()],
        },
        registered: showcase_epoch() - Duration::from_hours(9 * 24),
        nixpkgs: NixpkgsListing::Listed(NixpkgsEntry {
            definition: Definition {
                file: "pkgs/by-name/fo/foot/package.nix".to_owned(),
                url: "https://github.com/NixOS/nixpkgs/blob/nixos-unstable/pkgs/by-name/fo/foot/package.nix".to_owned(),
            },
            homepage: PackageHomepage::Listed("https://codeberg.org/dnkl/foot".to_owned()),
            license: PackageLicense::Spdx("MIT".to_owned()),
            generation: Generation::Rebuilt {
                version: "1.23.2".to_owned(),
            },
            pin: FlakePin::Moved {
                version: "1.23.4".to_owned(),
            },
        }),
        upstream: Upstream {
            branch: "nixos-unstable".to_owned(),
            state: UpstreamState::Newer {
                version: "1.24.0".to_owned(),
            },
        },
    }
}
