use std::path::{Path, PathBuf};

use bytesize::ByteSize;
use chrono::{DateTime, Utc};

use super::{Error, block_filesystem_mounts::KernelDevice, read_disk_value};

const SYS_FS_EXT4: &str = "/sys/fs/ext4";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ext4Counters {
    pub recorded_errors: RecordedErrors,
    pub lifetime_written: ByteSize,
    pub written_since_mount: ByteSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordedErrors {
    Clean,
    Recorded {
        count: u64,
        first_seen: DateTime<Utc>,
        last_seen: DateTime<Utc>,
    },
}

pub struct Ext4SysfsDirectory {
    dir: PathBuf,
}

impl Ext4SysfsDirectory {
    pub fn containing(devices: &[KernelDevice]) -> Result<Self, Error> {
        devices
            .iter()
            .map(|device| Path::new(SYS_FS_EXT4).join(&device.name))
            .find(|dir| dir.exists())
            .map(|dir| Ext4SysfsDirectory { dir })
            .ok_or_else(|| Error::NoFilesystemOn {
                fs_type: "ext4",
                device: devices
                    .first()
                    .map_or_else(String::new, |device| device.name.clone()),
            })
    }

    pub fn counters(&self) -> Result<Ext4Counters, Error> {
        let kibibytes = |attribute: &str| -> Result<ByteSize, Error> {
            read_disk_value::<u64>(&self.dir.join(attribute)).map(ByteSize::kib)
        };
        let moment = |attribute: &str| -> Result<DateTime<Utc>, Error> {
            let path = self.dir.join(attribute);
            let seconds = read_disk_value::<i64>(&path)?;
            DateTime::from_timestamp(seconds, 0)
                .ok_or_else(|| Error::Unparsable(path.display().to_string()))
        };
        let error_count = read_disk_value::<u64>(&self.dir.join("errors_count"))?;
        let recorded_errors = if error_count == 0 {
            RecordedErrors::Clean
        } else {
            RecordedErrors::Recorded {
                count: error_count,
                first_seen: moment("first_error_time")?,
                last_seen: moment("last_error_time")?,
            }
        };
        Ok(Ext4Counters {
            recorded_errors,
            lifetime_written: kibibytes("lifetime_write_kbytes")?,
            written_since_mount: kibibytes("session_write_kbytes")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use super::*;

    fn fixture_directory(name: &str, attributes: &[(&str, &str)]) -> Ext4SysfsDirectory {
        let dir = env::temp_dir().join(format!("sottomano-ext4-{name}-{}", process::id()));
        fs::create_dir_all(&dir).expect("fixture directory");
        attributes.iter().for_each(|(attribute, contents)| {
            fs::write(dir.join(attribute), contents).expect("fixture attribute");
        });
        Ext4SysfsDirectory { dir }
    }

    #[test]
    fn clean_filesystem_reports_written_kibibytes() {
        let directory = fixture_directory(
            "clean",
            &[
                ("errors_count", "0\n"),
                ("first_error_time", "0\n"),
                ("last_error_time", "0\n"),
                ("lifetime_write_kbytes", "2048\n"),
                ("session_write_kbytes", "16\n"),
            ],
        );
        let counters = directory.counters().expect("counters");
        assert_eq!(counters.recorded_errors, RecordedErrors::Clean);
        assert_eq!(counters.lifetime_written, ByteSize::mib(2));
        assert_eq!(counters.written_since_mount, ByteSize::kib(16));
    }

    #[test]
    fn recorded_errors_carry_their_first_and_last_moments() {
        let directory = fixture_directory(
            "errors",
            &[
                ("errors_count", "3\n"),
                ("first_error_time", "1700000000\n"),
                ("last_error_time", "1700000600\n"),
                ("lifetime_write_kbytes", "0\n"),
                ("session_write_kbytes", "0\n"),
            ],
        );
        let RecordedErrors::Recorded {
            count,
            first_seen,
            last_seen,
        } = directory.counters().expect("counters").recorded_errors
        else {
            panic!("errors should be recorded");
        };
        assert_eq!(count, 3);
        assert_eq!((last_seen - first_seen).num_seconds(), 600);
    }
}
