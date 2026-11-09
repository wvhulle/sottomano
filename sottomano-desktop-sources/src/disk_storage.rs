pub mod bcachefs_filesystem_space;
pub mod bcachefs_subvolume_usage;
pub mod block_filesystem_mounts;
pub mod btrfs_filesystem_space;
pub mod disk_io_activity;
pub mod drive_temperature_health;
pub mod ext4_sysfs_counters;
pub mod filesystem_device_errors;
pub mod mounted_filesystem_usage;
pub mod udev_volume_identity;
pub mod whole_disk_space_occupancy;

use std::{fs, path::Path, str::FromStr};

use crate::{blocking_background_work::Panicked, system_protocol_clients::shell_command_output};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("statvfs on {mount_point} failed (errno {errno})")]
    Statvfs { mount_point: String, errno: i32 },
    #[error("cannot read {0}")]
    Unreadable(String),
    #[error("unexpected contents in {0}")]
    Unparsable(String),
    #[error("no {fs_type} filesystem found on {device}")]
    NoFilesystemOn {
        fs_type: &'static str,
        device: String,
    },
    #[error("{0}")]
    Command(#[from] shell_command_output::Error),
    #[error("disk read panicked")]
    Panicked,
}

impl From<Panicked> for Error {
    fn from(Panicked: Panicked) -> Self {
        Error::Panicked
    }
}

pub(crate) fn unparsable_disk_file(path: &Path) -> Error {
    Error::Unparsable(path.display().to_string())
}

pub(crate) fn read_disk_text(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|_| Error::Unreadable(path.display().to_string()))
}

pub(crate) fn read_disk_value<T: FromStr>(path: &Path) -> Result<T, Error> {
    read_disk_text(path)?
        .trim()
        .parse()
        .map_err(|_| unparsable_disk_file(path))
}
