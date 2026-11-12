use std::path::PathBuf;

use bytesize::ByteSize;

use super::{
    Error,
    block_filesystem_mounts::{Compression, KernelDevice},
    filesystem_device_errors::{DeviceErrors, ErrorCount},
    read_disk_text, read_disk_value,
};
use crate::system_protocol_clients::sysfs_attribute_reads::list_dir;

const SYS_FS_BTRFS: &str = "/sys/fs/btrfs";
const SECTOR_BYTES: u64 = 512;
const BLOCK_GROUP_KINDS: [&str; 3] = ["data", "metadata", "system"];
const FALLBACK_METADATA_CHUNK: ByteSize = ByteSize::mib(256);
const ERROR_RESET_COMMAND: &str = "btrfs device stats -z /";

#[derive(Debug, Clone, PartialEq)]
pub struct BtrfsSpace {
    pub compression: Compression,
    pub unallocated: ByteSize,
    pub metadata: Allocation,
    pub global_reserve: ByteSize,
    pub metadata_chunk: ByteSize,
}

impl BtrfsSpace {
    #[must_use]
    pub fn metadata_slack(&self) -> ByteSize {
        ByteSize::b(
            self.metadata
                .allocated
                .as_u64()
                .saturating_sub(self.metadata.used.as_u64()),
        )
    }

    #[must_use]
    pub fn metadata_chunk_room(&self) -> ChunkRoom {
        if self.unallocated >= self.metadata_chunk {
            ChunkRoom::Allocatable
        } else {
            ChunkRoom::Exhausted
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    pub used: ByteSize,
    pub allocated: ByteSize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ErrorStats {
    write: u64,
    read: u64,
    flush: u64,
    corruption: u64,
    generation: u64,
}

pub struct BtrfsFilesystem {
    dir: PathBuf,
}

impl BtrfsFilesystem {
    pub fn containing(devices: &[KernelDevice]) -> Result<Self, Error> {
        list_dir(SYS_FS_BTRFS)
            .into_iter()
            .find(|dir| {
                devices
                    .iter()
                    .any(|device| dir.join("devices").join(&device.name).exists())
            })
            .map(|dir| BtrfsFilesystem { dir })
            .ok_or_else(|| Error::NoFilesystemOn {
                fs_type: "btrfs",
                device: devices
                    .first()
                    .map_or_else(String::new, |device| device.name.clone()),
            })
    }

    pub fn space(&self, compression: Compression) -> Result<BtrfsSpace, Error> {
        Ok(BtrfsSpace {
            compression,
            unallocated: self.unallocated()?,
            metadata: Allocation {
                used: self.allocation_bytes("metadata", "bytes_used")?,
                allocated: self.allocation_bytes("metadata", "total_bytes")?,
            },
            global_reserve: read_disk_value(&self.dir.join("allocation").join("global_rsv_size"))
                .map(ByteSize::b)?,
            metadata_chunk: self
                .allocation_bytes("metadata", "chunk_size")
                .unwrap_or(FALLBACK_METADATA_CHUNK),
        })
    }

    pub fn device_errors(&self) -> Result<DeviceErrors, Error> {
        let summed = list_dir(self.dir.join("devinfo"))
            .iter()
            .map(|device| {
                let path = device.join("error_stats");
                read_disk_text(&path).map(|stats| ErrorStats::parse(&stats))
            })
            .try_fold(ErrorStats::default(), |total, device| {
                device.map(|device| total.plus(device))
            })?;
        Ok(DeviceErrors {
            counted_on: self
                .dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned(),
            counts: summed.by_kind().to_vec(),
            reset_command: ERROR_RESET_COMMAND.to_owned(),
        })
    }

    fn unallocated(&self) -> Result<ByteSize, Error> {
        let device_sectors: u64 = list_dir(self.dir.join("devices"))
            .iter()
            .map(|device| read_disk_value::<u64>(&device.join("size")))
            .sum::<Result<_, _>>()?;
        let allocated: u64 = BLOCK_GROUP_KINDS
            .iter()
            .map(|kind| {
                self.allocation_bytes(kind, "disk_total")
                    .map(|bytes| bytes.as_u64())
            })
            .sum::<Result<_, _>>()?;
        Ok(ByteSize::b(
            (device_sectors * SECTOR_BYTES).saturating_sub(allocated),
        ))
    }

    fn allocation_bytes(&self, kind: &str, counter: &str) -> Result<ByteSize, Error> {
        read_disk_value(&self.dir.join("allocation").join(kind).join(counter)).map(ByteSize::b)
    }
}

impl ErrorStats {
    fn parse(stats: &str) -> Self {
        stats
            .lines()
            .filter_map(|line| {
                let (name, count) = line.split_once(' ')?;
                let parsed_count = count
                    .trim()
                    .parse()
                    .inspect_err(|parse_failure| {
                        tracing::warn!(
                            "btrfs: error stat {name} count {count} is malformed: {parse_failure}"
                        );
                    })
                    .ok()?;
                Some((name, parsed_count))
            })
            .fold(ErrorStats::default(), |errors, (name, count)| {
                errors.counted(name, count)
            })
    }

    fn counted(mut self, name: &str, count: u64) -> Self {
        match name {
            "write_errs" => self.write += count,
            "read_errs" => self.read += count,
            "flush_errs" => self.flush += count,
            "corruption_errs" => self.corruption += count,
            "generation_errs" => self.generation += count,
            _ => {}
        }
        self
    }

    fn plus(self, other: Self) -> Self {
        ErrorStats {
            write: self.write + other.write,
            read: self.read + other.read,
            flush: self.flush + other.flush,
            corruption: self.corruption + other.corruption,
            generation: self.generation + other.generation,
        }
    }

    fn by_kind(&self) -> [ErrorCount; 5] {
        [
            ErrorCount::new("Write", self.write),
            ErrorCount::new("Read", self.read),
            ErrorCount::new("Flush", self.flush),
            ErrorCount::new("Corruption", self.corruption),
            ErrorCount::new("Generation", self.generation),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkRoom {
    Allocatable,
    Exhausted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_stats_are_parsed_per_kind() {
        let errors = ErrorStats::parse(
            "write_errs 0\nread_errs 3\nflush_errs 0\ncorruption_errs 1\ngeneration_errs 0\n",
        );
        assert_eq!(errors.read, 3);
        assert_eq!(errors.corruption, 1);
        assert_eq!(
            errors
                .by_kind()
                .iter()
                .map(|error| error.count)
                .sum::<u64>(),
            4
        );
    }

    #[test]
    fn device_errors_sum_across_devices() {
        let one = ErrorStats {
            write: 1,
            ..ErrorStats::default()
        };
        assert_eq!(one.plus(one).write, 2);
    }
}
