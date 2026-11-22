use super::*;

const ALLOC_DEBUG: &str = "                     buckets         sectors      fragmented
free                  497252               0               0
sb                         2            4104            4088
journal                 4096        16777216               0
btree                   9900        11774976        28775424
user                  427139      1578139800       171421560
need_discard           38367               0               0
capacity              976756

reserves:
stripe                 30530
btree                      4
";

const IO_ERRORS: &str = "IO errors since filesystem creation
  read:    7
  write:   0
  checksum:2
IO errors since 8 y ago
  read:    0
  write:   0
  checksum:1
";

const COMPRESSION_STATS: &str = "typetype          compressed    uncompressed     average extent \
                                 size
lz4_old                    0               0                       0
zstd                    475G            831G                    172k
incompressible         1006G           1006G                    166k
";

fn device_with(free_gib: u64, allowed_data: AllowedDeviceData, state: &str) -> BcachefsDeviceSpace {
    BcachefsDeviceSpace {
        label: String::new(),
        partition: String::new(),
        state: state.to_owned(),
        free: ByteSize::gib(free_gib),
        capacity: ByteSize::gib(1000),
        allowed_data,
    }
}

fn space_with(devices: Vec<BcachefsDeviceSpace>, metadata_replicas: usize) -> BcachefsSpace {
    BcachefsSpace {
        devices,
        metadata_replicas,
        data_replicas: 1,
        compression: "zstd".to_owned(),
        background_compression: "none".to_owned(),
        compressed_extents: CompressedExtents::default(),
        snapshot_deletion: SnapshotDeletion::Idle,
    }
}

#[test]
fn bucket_counts_stop_before_the_reserves_section() {
    let buckets = BucketCounts::parse(ALLOC_DEBUG).unwrap();
    assert_eq!(buckets.reusable, 497_252 + 38_367);
    assert_eq!(buckets.capacity, 976_756);
}

#[test]
fn errors_are_counted_since_the_last_reset() {
    let counts = parse_errors_since_reset(IO_ERRORS);
    assert_eq!(counts[0], ErrorCount::new("Read", 0));
    assert_eq!(counts[2], ErrorCount::new("Checksum", 1));
}

#[test]
fn human_sizes_are_binary() {
    assert_eq!(parse_human_size("2.00M\n"), Some(2 << 20));
    assert_eq!(parse_human_size("512k"), Some(512 << 10));
    assert_eq!(parse_human_size("0"), Some(0));
    assert_eq!(parse_human_size("lots"), None);
}

#[test]
fn selected_state_is_the_bracketed_choice() {
    assert_eq!(selected_choice("[rw] ro evacuating spare\n"), Some("rw"));
    assert_eq!(selected_choice("rw [ro] evacuating spare"), Some("ro"));
}

#[test]
fn compression_totals_skip_incompressible_extents() {
    let extents = CompressedExtents::parse(COMPRESSION_STATS);
    assert_eq!(extents.compressed, ByteSize::gib(475));
    assert_eq!(extents.uncompressed, ByteSize::gib(831));
}

#[test]
fn single_replica_headroom_is_the_emptiest_metadata_device() {
    let space = space_with(
        vec![
            device_with(10, AllowedDeviceData::DataAndMetadata, "rw"),
            device_with(50, AllowedDeviceData::DataAndMetadata, "rw"),
        ],
        1,
    );
    assert_eq!(space.metadata_headroom(), ByteSize::gib(50));
}

#[test]
fn two_replicas_are_limited_by_the_second_emptiest_device() {
    let space = space_with(
        vec![
            device_with(10, AllowedDeviceData::DataAndMetadata, "rw"),
            device_with(50, AllowedDeviceData::DataAndMetadata, "rw"),
            device_with(900, AllowedDeviceData::DataOnly, "rw"),
        ],
        2,
    );
    assert_eq!(space.metadata_headroom(), ByteSize::gib(10));
}

#[test]
fn too_few_writable_metadata_devices_leave_no_headroom() {
    let space = space_with(
        vec![
            device_with(10, AllowedDeviceData::DataAndMetadata, "rw"),
            device_with(50, AllowedDeviceData::DataAndMetadata, "ro"),
        ],
        2,
    );
    assert_eq!(space.metadata_headroom(), ByteSize::b(0));
}

#[test]
fn three_replicas_take_the_third_largest() {
    let space = space_with(
        vec![
            device_with(30, AllowedDeviceData::DataAndMetadata, "rw"),
            device_with(10, AllowedDeviceData::DataAndMetadata, "rw"),
            device_with(20, AllowedDeviceData::DataAndMetadata, "rw"),
        ],
        3,
    );
    assert_eq!(space.metadata_headroom(), ByteSize::gib(10));
}
