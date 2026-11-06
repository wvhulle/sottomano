use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use bytesize::ByteSize;
use futures::Stream;
use rustix::fs::{StatVfsMountFlags, statvfs};
use serde::Deserialize;

use super::{Error, block_filesystem_mounts::FilesystemMount};
use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::shell_command_output,
};

const BCACHEFS_PROGRAM: &str = "bcachefs";
const SECTOR_BYTES: u64 = 512;
const INTERIOR_NODE_SUBVOLUME: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BcachefsSubvolumeUsage {
    pub subvolumes: Vec<BcachefsSubvolume>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BcachefsSubvolume {
    pub path: PathBuf,
    pub used: ByteSize,
    pub only_in_snapshots: ByteSize,
    pub snapshot_count: usize,
    pub mounts: Vec<SubvolumeMount>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubvolumeMount {
    pub mount_point: PathBuf,
    pub access: MountAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountAccess {
    ReadWrite,
    ReadOnly,
}

impl SnapshotSource for BcachefsSubvolumeUsage {
    type Error = Error;
    type StreamInputs = FilesystemMount;

    const NAME: &'static str = "Subvolume usage";

    fn delivery_stream(
        mount: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(60))
            .blocking_poll_deliveries(move || read_subvolume_usage(&mount))
    }
}

fn read_subvolume_usage(mount: &FilesystemMount) -> Result<BcachefsSubvolumeUsage, Error> {
    let mounted_trees = mount
        .mount_points
        .iter()
        .map(|mount_point| MountedSnapshotTree::read(mount_point))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(merge_mounted_trees(mounted_trees))
}

fn merge_mounted_trees(
    mounted_trees: impl IntoIterator<Item = MountedSnapshotTree>,
) -> BcachefsSubvolumeUsage {
    let by_subvolume = mounted_trees.into_iter().fold(
        BTreeMap::new(),
        |mut by_subvolume: BTreeMap<u32, BcachefsSubvolume>, mounted| {
            by_subvolume
                .entry(mounted.tree.query_root.subvol)
                .or_insert_with(|| {
                    mounted
                        .tree
                        .subvolume_mounted_at(&mounted.mount.mount_point)
                })
                .mounts
                .push(mounted.mount);
            by_subvolume
        },
    );
    let mut subvolumes: Vec<BcachefsSubvolume> = by_subvolume.into_values().collect();
    subvolumes.sort_by(|first, second| {
        second
            .used
            .cmp(&first.used)
            .then_with(|| first.path.cmp(&second.path))
    });
    BcachefsSubvolumeUsage { subvolumes }
}

struct MountedSnapshotTree {
    mount: SubvolumeMount,
    tree: SnapshotTreeListing,
}

impl MountedSnapshotTree {
    fn read(mount_point: &Path) -> Result<Self, Error> {
        let mount_point_text = mount_point.to_string_lossy();
        let listing = shell_command_output::blocking::output(
            BCACHEFS_PROGRAM,
            &["subvolume", "list-snapshots", "--json", &mount_point_text],
        )?;
        let tree = serde_json::from_str(&listing).map_err(|_| {
            Error::Unparsable(format!("{BCACHEFS_PROGRAM} output for {mount_point_text}"))
        })?;
        Ok(MountedSnapshotTree {
            mount: SubvolumeMount {
                mount_point: mount_point.to_path_buf(),
                access: mount_access(mount_point)?,
            },
            tree,
        })
    }
}

fn mount_access(mount_point: &Path) -> Result<MountAccess, Error> {
    let stat = statvfs(mount_point).map_err(|errno| Error::Statvfs {
        mount_point: mount_point.display().to_string(),
        errno: errno.raw_os_error(),
    })?;
    Ok(if stat.f_flag.contains(StatVfsMountFlags::RDONLY) {
        MountAccess::ReadOnly
    } else {
        MountAccess::ReadWrite
    })
}

#[derive(Deserialize)]
struct SnapshotTreeListing {
    query_root: SnapshotTreeRoot,
    nodes: Vec<SnapshotTreeNode>,
}

#[derive(Deserialize)]
struct SnapshotTreeRoot {
    subvol: u32,
    path: Option<PathBuf>,
}

#[derive(Deserialize)]
struct SnapshotTreeNode {
    subvol: u32,
    sectors: u64,
}

impl SnapshotTreeListing {
    fn subvolume_mounted_at(&self, mount_point: &Path) -> BcachefsSubvolume {
        let snapshot_nodes = || self.nodes.iter().filter(|node| self.is_snapshot(node));
        BcachefsSubvolume {
            path: self
                .query_root
                .path
                .clone()
                .unwrap_or_else(|| mount_point.to_path_buf()),
            used: sectors_to_size(self.nodes.iter()),
            only_in_snapshots: sectors_to_size(snapshot_nodes()),
            snapshot_count: snapshot_nodes().count(),
            mounts: Vec::new(),
        }
    }

    fn is_snapshot(&self, node: &SnapshotTreeNode) -> bool {
        node.subvol != INTERIOR_NODE_SUBVOLUME && node.subvol != self.query_root.subvol
    }
}

fn sectors_to_size<'a>(nodes: impl Iterator<Item = &'a SnapshotTreeNode>) -> ByteSize {
    ByteSize::b(nodes.map(|node| node.sectors).sum::<u64>() * SECTOR_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE_ONLY: &str = r#"{
        "nodes": [{
            "children": [], "id": 4294967294, "key_bytes": 2550387496, "nr_keys": 25058837,
            "parent": 0, "path": "/home", "sectors": 2048, "size": "1.00M", "subvol": 2
        }],
        "query_root": {"path": "/home", "snapshot": 4294967294, "subvol": 2}
    }"#;

    const WITH_SNAPSHOTS: &str = r#"{
        "nodes": [
            {"children": [11, 12], "id": 10, "key_bytes": 0, "nr_keys": 0, "parent": 0,
             "sectors": 4096, "size": "2.00M", "subvol": 0},
            {"children": [], "id": 11, "key_bytes": 0, "nr_keys": 0, "parent": 10,
             "path": "/home", "sectors": 2048, "size": "1.00M", "subvol": 2},
            {"children": [], "id": 12, "key_bytes": 0, "nr_keys": 0, "parent": 10,
             "path": "/snapshots/home", "sectors": 1024, "size": "512k", "subvol": 7,
             "flags": "ro,snap"}
        ],
        "query_root": {"path": "/home", "snapshot": 10, "subvol": 2}
    }"#;

    fn mounted(listing: &str, mount_point: &str, access: MountAccess) -> MountedSnapshotTree {
        MountedSnapshotTree {
            mount: SubvolumeMount {
                mount_point: PathBuf::from(mount_point),
                access,
            },
            tree: serde_json::from_str(listing).unwrap(),
        }
    }

    fn listing_of(path: &str, subvol: u32, sectors: u64) -> String {
        format!(
            r#"{{"nodes": [{{"subvol": {subvol}, "sectors": {sectors}}}],
                "query_root": {{"path": "{path}", "subvol": {subvol}}}}}"#
        )
    }

    #[test]
    fn subvolume_without_snapshots_reports_its_own_sectors() {
        let usage = merge_mounted_trees([mounted(LIVE_ONLY, "/home", MountAccess::ReadWrite)]);
        let home = &usage.subvolumes[0];
        assert_eq!(home.path, Path::new("/home"));
        assert_eq!(home.used, ByteSize::mib(1));
        assert_eq!(home.snapshot_count, 0);
        assert_eq!(home.only_in_snapshots, ByteSize::b(0));
    }

    #[test]
    fn interior_nodes_count_towards_use_but_not_as_snapshots() {
        let usage = merge_mounted_trees([mounted(WITH_SNAPSHOTS, "/home", MountAccess::ReadWrite)]);
        let home = &usage.subvolumes[0];
        assert_eq!(home.used, ByteSize::kib(3584));
        assert_eq!(home.snapshot_count, 1);
        assert_eq!(home.only_in_snapshots, ByteSize::kib(512));
    }

    #[test]
    fn mounts_of_one_subvolume_merge_into_one_row() {
        let nix = listing_of("/nix", 5, 2048);
        let usage = merge_mounted_trees([
            mounted(&nix, "/nix", MountAccess::ReadWrite),
            mounted(&nix, "/nix/store", MountAccess::ReadOnly),
        ]);
        assert_eq!(usage.subvolumes.len(), 1);
        assert_eq!(usage.subvolumes[0].used, ByteSize::mib(1));
        assert_eq!(
            usage.subvolumes[0].mounts,
            vec![
                SubvolumeMount {
                    mount_point: PathBuf::from("/nix"),
                    access: MountAccess::ReadWrite,
                },
                SubvolumeMount {
                    mount_point: PathBuf::from("/nix/store"),
                    access: MountAccess::ReadOnly,
                },
            ]
        );
    }

    #[test]
    fn largest_subvolume_comes_first() {
        let usage = merge_mounted_trees([
            mounted(&listing_of("/root", 1, 10), "/", MountAccess::ReadWrite),
            mounted(&listing_of("/home", 2, 90), "/home", MountAccess::ReadWrite),
        ]);
        assert_eq!(usage.subvolumes[0].path, Path::new("/home"));
        assert_eq!(usage.subvolumes[1].path, Path::new("/root"));
    }

    #[test]
    fn unresolvable_subvolume_path_falls_back_to_the_mount_point() {
        let listing = r#"{"nodes": [{"subvol": 3, "sectors": 8}], "query_root": {"subvol": 3}}"#;
        let usage = merge_mounted_trees([mounted(listing, "/data", MountAccess::ReadWrite)]);
        assert_eq!(usage.subvolumes[0].path, Path::new("/data"));
    }

    #[test]
    fn malformed_listing_is_rejected() {
        assert!(serde_json::from_str::<SnapshotTreeListing>("[]").is_err());
    }
}
