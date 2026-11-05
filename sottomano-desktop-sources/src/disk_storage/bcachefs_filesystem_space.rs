use std::{
    fs,
    path::{Path, PathBuf},
};

use bytesize::ByteSize;

use super::{
    Error,
    block_filesystem_mounts::KernelDevice,
    filesystem_device_errors::{DeviceErrors, ErrorCount},
};
use crate::system_protocol_clients::sysfs_attribute_reads::list_dir;

const SYS_FS_BCACHEFS: &str = "/sys/fs/bcachefs";
const MEMBER_DIR_PREFIX: &str = "dev-";
const METADATA_DATA_TYPE: &str = "btree";
const WRITABLE_STATE: &str = "rw";
const IDLE_SNAPSHOT_DELETION: &str = "(not running)";
const INCOMPRESSIBLE_ROW: &str = "incompressible";
const ERRORS_SINCE_HEADER: &str = "IO errors since";

#[derive(Debug, Clone, PartialEq)]
pub struct BcachefsSpace {
    pub devices: Vec<BcachefsDeviceSpace>,
    pub metadata_replicas: usize,
    pub data_replicas: usize,
    pub compression: String,
    pub background_compression: String,
    pub compressed_extents: CompressedExtents,
    pub snapshot_deletion: SnapshotDeletion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BcachefsDeviceSpace {
    pub label: String,
    pub partition: String,
    pub state: String,
    pub free: ByteSize,
    pub capacity: ByteSize,
    pub holds_metadata: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompressedExtents {
    pub compressed: ByteSize,
    pub uncompressed: ByteSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotDeletion {
    Idle,
    Running,
}

impl BcachefsSpace {
    #[must_use]
    pub fn metadata_headroom(&self) -> ByteSize {
        let mut metadata_free: Vec<ByteSize> = self
            .devices
            .iter()
            .filter(|device| device.accepts_metadata_writes())
            .map(|device| device.free)
            .collect();
        metadata_free.sort_unstable_by(|first, second| second.cmp(first));
        self.metadata_replicas
            .checked_sub(1)
            .and_then(|nth_largest| metadata_free.get(nth_largest).copied())
            .unwrap_or_default()
    }
}

impl BcachefsDeviceSpace {
    fn accepts_metadata_writes(&self) -> bool {
        self.holds_metadata && self.state == WRITABLE_STATE
    }
}

pub struct BcachefsFilesystem {
    dir: PathBuf,
}

struct BcachefsMember {
    dir: PathBuf,
    partition: String,
}

impl BcachefsFilesystem {
    pub fn containing(devices: &[KernelDevice]) -> Result<Self, Error> {
        let first_device = devices.first().map_or("", |device| device.name.as_str());
        list_dir(SYS_FS_BCACHEFS)
            .into_iter()
            .map(|dir| BcachefsFilesystem { dir })
            .find(|filesystem| {
                filesystem
                    .members()
                    .iter()
                    .any(|member| devices.iter().any(|device| device.name == member.partition))
            })
            .ok_or_else(|| Error::NoFilesystemOn {
                fs_type: "bcachefs",
                device: first_device.to_owned(),
            })
    }

    pub fn space(&self) -> Result<BcachefsSpace, Error> {
        Ok(BcachefsSpace {
            devices: self
                .members()
                .iter()
                .map(BcachefsMember::space)
                .collect::<Result<_, _>>()?,
            metadata_replicas: read_parsed(&self.option_path("metadata_replicas"))?,
            data_replicas: read_parsed(&self.option_path("data_replicas"))?,
            compression: read_trimmed_text(&self.option_path("compression"))?,
            background_compression: read_trimmed_text(&self.option_path("background_compression"))?,
            compressed_extents: CompressedExtents::parse(&read_text(
                &self.dir.join("compression_stats"),
            )?),
            snapshot_deletion: SnapshotDeletion::parse(&read_text(
                &self.dir.join("snapshot_delete_status"),
            )?),
        })
    }

    pub fn device_errors(&self, partition: &KernelDevice) -> Result<DeviceErrors, Error> {
        let member = self
            .members()
            .into_iter()
            .find(|member| member.partition == partition.name)
            .ok_or_else(|| Error::NoFilesystemOn {
                fs_type: "bcachefs",
                device: partition.name.clone(),
            })?;
        let path = member.dir.join("io_errors");
        Ok(DeviceErrors {
            counted_on: member.partition.clone(),
            counts: parse_errors_since_reset(&read_text(&path)?),
            reset_command: format!(
                "echo 1 | sudo tee {}",
                member.dir.join("io_errors_reset").display()
            ),
        })
    }

    fn members(&self) -> Vec<BcachefsMember> {
        let mut members: Vec<BcachefsMember> = list_dir(&self.dir)
            .into_iter()
            .filter(|dir| {
                dir.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(MEMBER_DIR_PREFIX))
            })
            .filter_map(BcachefsMember::at)
            .collect();
        members.sort_by(|first, second| first.dir.cmp(&second.dir));
        members
    }

    fn option_path(&self, option: &str) -> PathBuf {
        self.dir.join("options").join(option)
    }
}

impl BcachefsMember {
    fn at(dir: PathBuf) -> Option<Self> {
        let block = fs::read_link(dir.join("block")).ok()?;
        let partition = block.file_name()?.to_str()?.to_owned();
        Some(BcachefsMember { dir, partition })
    }

    fn space(&self) -> Result<BcachefsDeviceSpace, Error> {
        let bucket_size_path = self.dir.join("bucket_size");
        let bucket_bytes = parse_human_size(&read_text(&bucket_size_path)?)
            .ok_or_else(|| unparsable(&bucket_size_path))?;
        let alloc_path = self.dir.join("alloc_debug");
        let buckets =
            BucketCounts::parse(&read_text(&alloc_path)?).ok_or_else(|| unparsable(&alloc_path))?;
        let state_path = self.dir.join("state");
        let label = read_trimmed_text(&self.dir.join("label"))?;
        Ok(BcachefsDeviceSpace {
            label: if label.is_empty() {
                self.partition.clone()
            } else {
                label
            },
            partition: self.partition.clone(),
            state: selected_choice(&read_text(&state_path)?)
                .ok_or_else(|| unparsable(&state_path))?
                .to_owned(),
            free: ByteSize::b(buckets.reusable * bucket_bytes),
            capacity: ByteSize::b(buckets.capacity * bucket_bytes),
            holds_metadata: read_text(&self.dir.join("data_allowed"))?
                .trim()
                .split(',')
                .any(|data_type| data_type == METADATA_DATA_TYPE),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BucketCounts {
    reusable: u64,
    capacity: u64,
}

impl BucketCounts {
    fn parse(alloc_debug: &str) -> Option<Self> {
        let counter = |name: &str| -> Option<u64> {
            alloc_debug
                .lines()
                .take_while(|line| !line.trim().is_empty())
                .find_map(|line| {
                    let mut fields = line.split_whitespace();
                    (fields.next()? == name).then(|| fields.next()?.parse().ok())?
                })
        };
        Some(BucketCounts {
            reusable: counter("free")? + counter("need_discard").unwrap_or(0),
            capacity: counter("capacity")?,
        })
    }
}

impl CompressedExtents {
    fn parse(compression_stats: &str) -> Self {
        compression_stats
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                let algorithm = fields.next()?;
                if algorithm == INCOMPRESSIBLE_ROW {
                    return None;
                }
                Some(CompressedExtents {
                    compressed: ByteSize::b(parse_human_size(fields.next()?)?),
                    uncompressed: ByteSize::b(parse_human_size(fields.next()?)?),
                })
            })
            .fold(CompressedExtents::default(), |total, row| {
                CompressedExtents {
                    compressed: total.compressed + row.compressed,
                    uncompressed: total.uncompressed + row.uncompressed,
                }
            })
    }
}

impl SnapshotDeletion {
    fn parse(status: &str) -> Self {
        if status.trim() == IDLE_SNAPSHOT_DELETION {
            SnapshotDeletion::Idle
        } else {
            SnapshotDeletion::Running
        }
    }
}

fn parse_errors_since_reset(io_errors: &str) -> Vec<ErrorCount> {
    let since_reset = io_errors
        .rsplit_once(ERRORS_SINCE_HEADER)
        .map_or(io_errors, |(_, latest_block)| latest_block);
    let count_of = |kind: &str| {
        since_reset
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.trim() == kind)
            .and_then(|(_, count)| count.trim().parse().ok())
            .unwrap_or(0)
    };
    vec![
        ErrorCount::new("Read", count_of("read")),
        ErrorCount::new("Write", count_of("write")),
        ErrorCount::new("Checksum", count_of("checksum")),
    ]
}

fn selected_choice(choices: &str) -> Option<&str> {
    let (_, after_open) = choices.split_once('[')?;
    let (selected, _) = after_open.split_once(']')?;
    Some(selected)
}

fn parse_human_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let (number, multiplier) = match text.chars().last()? {
        'k' | 'K' => (&text[..text.len() - 1], 1u64 << 10),
        'M' => (&text[..text.len() - 1], 1 << 20),
        'G' => (&text[..text.len() - 1], 1 << 30),
        'T' => (&text[..text.len() - 1], 1 << 40),
        'P' => (&text[..text.len() - 1], 1 << 50),
        _ => (text, 1),
    };
    let value: f64 = number.parse().ok()?;
    Some((value * multiplier as f64).round() as u64)
}

fn unparsable(path: &Path) -> Error {
    Error::Unparsable(path.display().to_string())
}

fn read_text(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|_| Error::Unreadable(path.display().to_string()))
}

fn read_trimmed_text(path: &Path) -> Result<String, Error> {
    read_text(path).map(|text| text.trim().to_owned())
}

fn read_parsed(path: &Path) -> Result<usize, Error> {
    read_text(path)?
        .trim()
        .parse()
        .map_err(|_| unparsable(path))
}

#[cfg(test)]
mod tests {
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

    const COMPRESSION_STATS: &str = "typetype          compressed    uncompressed     average \
                                     extent size
lz4_old                    0               0                       0
zstd                    475G            831G                    172k
incompressible         1006G           1006G                    166k
";

    fn device_with(free_gib: u64, holds_metadata: bool, state: &str) -> BcachefsDeviceSpace {
        BcachefsDeviceSpace {
            label: String::new(),
            partition: String::new(),
            state: state.to_owned(),
            free: ByteSize::gib(free_gib),
            capacity: ByteSize::gib(1000),
            holds_metadata,
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
            vec![device_with(10, true, "rw"), device_with(50, true, "rw")],
            1,
        );
        assert_eq!(space.metadata_headroom(), ByteSize::gib(50));
    }

    #[test]
    fn two_replicas_are_limited_by_the_second_emptiest_device() {
        let space = space_with(
            vec![
                device_with(10, true, "rw"),
                device_with(50, true, "rw"),
                device_with(900, false, "rw"),
            ],
            2,
        );
        assert_eq!(space.metadata_headroom(), ByteSize::gib(10));
    }

    #[test]
    fn too_few_writable_metadata_devices_leave_no_headroom() {
        let space = space_with(
            vec![device_with(10, true, "rw"), device_with(50, true, "ro")],
            2,
        );
        assert_eq!(space.metadata_headroom(), ByteSize::b(0));
    }

    #[test]
    fn three_replicas_take_the_third_largest() {
        let space = space_with(
            vec![
                device_with(30, true, "rw"),
                device_with(10, true, "rw"),
                device_with(20, true, "rw"),
            ],
            3,
        );
        assert_eq!(space.metadata_headroom(), ByteSize::gib(10));
    }
}
