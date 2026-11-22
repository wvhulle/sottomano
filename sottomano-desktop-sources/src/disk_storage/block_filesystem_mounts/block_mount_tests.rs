use super::*;

fn rendered_compression(options: &[(&str, Option<&str>)]) -> String {
    let options = options
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.map(str::to_owned)))
        .collect();
    Compression::from(&options).to_string()
}

#[test]
fn algorithm_with_level() {
    assert_eq!(
        rendered_compression(&[("compress", Some("zstd:3"))]),
        "zstd:3"
    );
}

#[test]
fn forced_compression_is_marked() {
    assert_eq!(
        rendered_compression(&[("compress-force", Some("lzo"))]),
        "lzo (forced)"
    );
}

#[test]
fn bare_compress_uses_the_btrfs_default() {
    assert_eq!(rendered_compression(&[("compress", None)]), "zlib");
}

#[test]
fn absent_or_no_means_off() {
    assert_eq!(rendered_compression(&[("ssd", None)]), "off");
    assert_eq!(rendered_compression(&[("compress", Some("no"))]), "off");
}

fn mount_entry(mount_point: &str, fs_type: &str, source: &str) -> MountEntry {
    MountEntry {
        mount_point: PathBuf::from(mount_point),
        fs_type: fs_type.to_owned(),
        source: source.to_owned(),
        options: HashMap::new(),
    }
}

fn device_named_after_node(source: &str) -> Option<KernelDevice> {
    Some(KernelDevice {
        name: source.strip_prefix("/dev/")?.to_owned(),
    })
}

#[test]
fn subvolume_mounts_collapse_into_one_filesystem() {
    let filesystems = group_block_mounts(
        [
            mount_entry("/home", "bcachefs", "/dev/nvme1n1p1:/dev/nvme0n1p3"),
            mount_entry("/", "bcachefs", "/dev/nvme1n1p1:/dev/nvme0n1p3"),
            mount_entry("/nix/store", "bcachefs", "/dev/nvme1n1p1:/dev/nvme0n1p3"),
            mount_entry("/boot", "vfat", "/dev/nvme0n1p1"),
            mount_entry("/run", "tmpfs", "tmpfs"),
            mount_entry("/mnt/iso", "iso9660", "/dev/loop0"),
        ],
        device_named_after_node,
    );
    assert_eq!(filesystems.len(), 2);
    let root = &filesystems[0];
    assert_eq!(root.primary_mount_point(), Path::new("/"));
    assert_eq!(root.mount_points.len(), 3);
    assert_eq!(root.devices.len(), 2);
    assert_eq!(root.mount_role(), MountRole::Root);
    assert_eq!(filesystems[1].primary_mount_point(), Path::new("/boot"));
}

#[test]
fn escaped_spaces_in_mount_points_are_restored() {
    assert_eq!(
        unescape_mountinfo_octal(Path::new("/run/media/user/4TB\\040Back-up")),
        PathBuf::from("/run/media/user/4TB Back-up")
    );
}

#[test]
fn device_order_in_the_source_does_not_split_a_filesystem() {
    let filesystems = group_block_mounts(
        [
            mount_entry("/", "bcachefs", "/dev/sda1:/dev/sdb1"),
            mount_entry("/home", "bcachefs", "/dev/sdb1:/dev/sda1"),
        ],
        device_named_after_node,
    );
    assert_eq!(filesystems.len(), 1);
}
