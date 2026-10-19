use std::{
    fs,
    iter::Sum,
    path::{Path, PathBuf},
};

use bytesize::ByteSize;

use super::{
    Error,
    root_mount_facts::{Compression, KernelDevice},
};
use crate::system_protocol_clients::sysfs_attribute_reads::list_dir;

const SYS_FS_BTRFS: &str = "/sys/fs/btrfs";
const SECTOR_BYTES: u64 = 512;
const BLOCK_GROUP_KINDS: [&str; 3] = ["data", "metadata", "system"];

#[derive(Debug, Clone, PartialEq)]
pub struct BtrfsSpace {
    pub compression: Compression,
    pub unallocated: ByteSize,
    pub metadata: Allocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    pub used: ByteSize,
    pub allocated: ByteSize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceErrors {
    pub write: u64,
    pub read: u64,
    pub flush: u64,
    pub corruption: u64,
    pub generation: u64,
}

pub struct BtrfsFilesystem {
    dir: PathBuf,
}

impl BtrfsFilesystem {
    pub fn containing(device: &KernelDevice) -> Result<Self, Error> {
        list_dir(SYS_FS_BTRFS)
            .into_iter()
            .find(|dir| dir.join("devices").join(&device.name).exists())
            .map(|dir| BtrfsFilesystem { dir })
            .ok_or_else(|| Error::NoBtrfsFilesystemOn(device.name.clone()))
    }

    pub fn space(&self, compression: Compression) -> Result<BtrfsSpace, Error> {
        Ok(BtrfsSpace {
            compression,
            unallocated: self.unallocated()?,
            metadata: Allocation {
                used: self.allocation_bytes("metadata", "bytes_used")?,
                allocated: self.allocation_bytes("metadata", "total_bytes")?,
            },
        })
    }

    pub fn device_errors(&self) -> Result<DeviceErrors, Error> {
        list_dir(self.dir.join("devinfo"))
            .iter()
            .map(|device| {
                let path = device.join("error_stats");
                read_text(&path).map(|stats| DeviceErrors::parse(&stats))
            })
            .sum()
    }

    fn unallocated(&self) -> Result<ByteSize, Error> {
        let device_sectors: u64 = list_dir(self.dir.join("devices"))
            .iter()
            .map(|device| read_u64(&device.join("size")))
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
        read_u64(&self.dir.join("allocation").join(kind).join(counter)).map(ByteSize::b)
    }
}

impl DeviceErrors {
    fn parse(stats: &str) -> Self {
        stats
            .lines()
            .filter_map(|line| {
                let (name, count) = line.split_once(' ')?;
                Some((name, count.trim().parse().ok()?))
            })
            .fold(DeviceErrors::default(), |errors, (name, count)| {
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

    #[must_use]
    pub fn by_kind(&self) -> [ErrorCount; 5] {
        [
            ErrorCount::new("Write", self.write),
            ErrorCount::new("Read", self.read),
            ErrorCount::new("Flush", self.flush),
            ErrorCount::new("Corruption", self.corruption),
            ErrorCount::new("Generation", self.generation),
        ]
    }

    #[must_use]
    pub fn total(&self) -> u64 {
        self.by_kind().iter().map(|error| error.count).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCount {
    pub kind: &'static str,
    pub count: u64,
}

impl ErrorCount {
    fn new(kind: &'static str, count: u64) -> Self {
        ErrorCount { kind, count }
    }
}

impl Sum for DeviceErrors {
    fn sum<I: Iterator<Item = Self>>(devices: I) -> Self {
        devices.fold(DeviceErrors::default(), |total, device| DeviceErrors {
            write: total.write + device.write,
            read: total.read + device.read,
            flush: total.flush + device.flush,
            corruption: total.corruption + device.corruption,
            generation: total.generation + device.generation,
        })
    }
}

fn read_text(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|_| Error::Unreadable(path.display().to_string()))
}

fn read_u64(path: &Path) -> Result<u64, Error> {
    read_text(path)?
        .trim()
        .parse()
        .map_err(|_| Error::Unparsable(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_stats_are_parsed_per_kind() {
        let errors = DeviceErrors::parse(
            "write_errs 0\nread_errs 3\nflush_errs 0\ncorruption_errs 1\ngeneration_errs 0\n",
        );
        assert_eq!(errors.read, 3);
        assert_eq!(errors.corruption, 1);
        assert_eq!(errors.total(), 4);
    }

    #[test]
    fn device_errors_sum_across_devices() {
        let one = DeviceErrors {
            write: 1,
            ..DeviceErrors::default()
        };
        let total: DeviceErrors = [one, one].into_iter().sum();
        assert_eq!(total.write, 2);
    }
}
