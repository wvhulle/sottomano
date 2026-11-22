use std::{fs, path::PathBuf};

use bytesize::ByteSize;

use super::{
    block_filesystem_mounts::KernelDevice,
    filesystem_device_errors::{DeviceErrors, ErrorCount},
};
use crate::system_protocol_clients::sysfs_attribute_reads::{
    FileReadError, list_dir, parse_field, read_field, unparsable_file,
};

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
    pub allowed_data: AllowedDeviceData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedDeviceData {
    DataAndMetadata,
    DataOnly,
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
            .filter(|device| {
                device.allowed_data == AllowedDeviceData::DataAndMetadata
                    && device.state == WRITABLE_STATE
            })
            .map(|device| device.free)
            .collect();
        metadata_free.sort_unstable_by(|first, second| second.cmp(first));
        self.metadata_replicas
            .checked_sub(1)
            .and_then(|nth_largest| metadata_free.get(nth_largest).copied())
            .unwrap_or_default()
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
    pub fn containing(devices: &[KernelDevice]) -> Result<Self, FileReadError> {
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
            .ok_or_else(|| FileReadError::NoFilesystemOn {
                fs_type: "bcachefs",
                device: first_device.to_owned(),
            })
    }

    pub fn space(&self) -> Result<BcachefsSpace, FileReadError> {
        Ok(BcachefsSpace {
            devices: self
                .members()
                .iter()
                .map(BcachefsMember::space)
                .collect::<Result<_, _>>()?,
            metadata_replicas: parse_field(&self.option_path("metadata_replicas"))?,
            data_replicas: parse_field(&self.option_path("data_replicas"))?,
            compression: parse_field::<String>(&self.option_path("compression"))?,
            background_compression: parse_field::<String>(
                &self.option_path("background_compression"),
            )?,
            compressed_extents: CompressedExtents::parse(&read_field(
                &self.dir.join("compression_stats"),
            )?),
            snapshot_deletion: SnapshotDeletion::parse(&read_field(
                &self.dir.join("snapshot_delete_status"),
            )?),
        })
    }

    pub fn device_errors(&self, partition: &KernelDevice) -> Result<DeviceErrors, FileReadError> {
        let member = self
            .members()
            .into_iter()
            .find(|member| member.partition == partition.name)
            .ok_or_else(|| FileReadError::NoFilesystemOn {
                fs_type: "bcachefs",
                device: partition.name.clone(),
            })?;
        let path = member.dir.join("io_errors");
        Ok(DeviceErrors {
            counted_on: member.partition.clone(),
            counts: parse_errors_since_reset(&read_field(&path)?),
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
        let block = fs::read_link(dir.join("block"))
            .inspect_err(|read_failure| {
                tracing::debug!(
                    "bcachefs: block link of {} unreadable: {read_failure}",
                    dir.display()
                );
            })
            .ok()?;
        let partition = block.file_name()?.to_str()?.to_owned();
        Some(BcachefsMember { dir, partition })
    }

    fn space(&self) -> Result<BcachefsDeviceSpace, FileReadError> {
        let bucket_size_path = self.dir.join("bucket_size");
        let bucket_bytes = parse_human_size(&read_field(&bucket_size_path)?)
            .ok_or_else(|| unparsable_file(&bucket_size_path))?;
        let alloc_path = self.dir.join("alloc_debug");
        let buckets = BucketCounts::parse(&read_field(&alloc_path)?)
            .filter(|bucket_counts| bucket_counts.reusable <= bucket_counts.capacity)
            .ok_or_else(|| unparsable_file(&alloc_path))?;
        let state_path = self.dir.join("state");
        let label = parse_field::<String>(&self.dir.join("label"))?;
        Ok(BcachefsDeviceSpace {
            label: if label.is_empty() {
                self.partition.clone()
            } else {
                label
            },
            partition: self.partition.clone(),
            state: selected_choice(&read_field(&state_path)?)
                .ok_or_else(|| unparsable_file(&state_path))?
                .to_owned(),
            free: ByteSize::b(buckets.reusable * bucket_bytes),
            capacity: ByteSize::b(buckets.capacity * bucket_bytes),
            allowed_data: if read_field(&self.dir.join("data_allowed"))?
                .trim()
                .split(',')
                .any(|data_type| data_type == METADATA_DATA_TYPE)
            {
                AllowedDeviceData::DataAndMetadata
            } else {
                AllowedDeviceData::DataOnly
            },
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
                    (fields.next()? == name).then(|| {
                        let count_text = fields.next()?;
                        count_text
                            .parse()
                            .inspect_err(|parse_failure| {
                                tracing::warn!(
                                    "bcachefs: alloc_debug {name} count {count_text} is \
                                     malformed: {parse_failure}"
                                );
                            })
                            .ok()
                    })?
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
            .and_then(|(_, count)| {
                count
                    .trim()
                    .parse()
                    .inspect_err(|parse_failure| {
                        tracing::warn!(
                            "bcachefs: io_errors {kind} count {count} is malformed: \
                             {parse_failure}"
                        );
                    })
                    .ok()
            })
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
    let value: f64 = number
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!("bcachefs: size {text} is malformed: {parse_failure}");
        })
        .ok()?;
    Some((value * multiplier as f64).round() as u64)
}

#[cfg(test)]
mod bcachefs_space_tests;
