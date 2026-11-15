use std::{
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

use uom::si::{f64::Frequency, frequency::kilohertz};

use crate::{
    blocking_background_work::Panicked, delivery_cadence_drivers::ChangeWatchFailure,
    system_protocol_clients::shell_command_output,
};

#[derive(Debug, thiserror::Error)]
pub enum FileReadError {
    #[error("change watch: {0}")]
    ChangeWatch(#[from] ChangeWatchFailure),
    #[error("cannot read {0}")]
    Unreadable(String),
    #[error("cannot write {path}: {detail}")]
    Unwritable { path: String, detail: String },
    #[error("unexpected contents in {0}")]
    Unparsable(String),
    #[error("statvfs on {mount_point} failed (errno {errno})")]
    Statvfs { mount_point: String, errno: i32 },
    #[error("no {fs_type} filesystem found on {device}")]
    NoFilesystemOn {
        fs_type: &'static str,
        device: String,
    },
    #[error("{0}")]
    Command(#[from] shell_command_output::Error),
    #[error("read panicked")]
    Panicked,
}

impl From<Panicked> for FileReadError {
    fn from(Panicked: Panicked) -> Self {
        FileReadError::Panicked
    }
}

pub fn read_field(path: &Path) -> Result<String, FileReadError> {
    fs::read_to_string(path)
        .map(|field_text| field_text.trim().to_string())
        .map_err(|read_failure| {
            FileReadError::Unreadable(format!("{}: {read_failure}", path.display()))
        })
}

pub fn parse_field<T: FromStr>(path: &Path) -> Result<T, FileReadError> {
    read_field(path)?.parse().map_err(|_| unparsable_file(path))
}

#[must_use]
pub fn unparsable_file(path: &Path) -> FileReadError {
    FileReadError::Unparsable(path.display().to_string())
}

#[must_use]
pub fn read_kilohertz(path: impl AsRef<Path>) -> Option<Frequency> {
    read_i64(path).map(|khz| Frequency::new::<kilohertz>(khz as f64))
}

#[must_use]
pub fn list_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let directory = dir.as_ref();
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(list_failure) if list_failure.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(list_failure) => {
            tracing::warn!("sysfs: cannot list {}: {list_failure}", directory.display());
            return Vec::new();
        }
    };
    entries
        .filter_map(|entry| {
            entry
                .inspect_err(|entry_failure| {
                    tracing::warn!(
                        "sysfs: unreadable entry in {}: {entry_failure}",
                        directory.display()
                    );
                })
                .ok()
                .map(|entry| entry.path())
        })
        .collect()
}

#[must_use]
pub fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    let attribute_path = path.as_ref();
    match fs::read_to_string(attribute_path) {
        Ok(attribute_text) => Some(attribute_text.trim().to_string()),
        Err(read_failure) if read_failure.kind() == io::ErrorKind::NotFound => None,
        Err(read_failure) => {
            tracing::debug!(
                "sysfs: cannot read {}: {read_failure}",
                attribute_path.display()
            );
            None
        }
    }
}

#[must_use]
pub fn read_i64(path: impl AsRef<Path>) -> Option<i64> {
    let attribute_path = path.as_ref();
    let attribute_text = read_trimmed(attribute_path)?;
    attribute_text
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "sysfs: {} holds {attribute_text:?}, not an integer: {parse_failure}",
                attribute_path.display()
            );
        })
        .ok()
}

#[must_use]
pub fn uevent_field(uevent: &Path, field: &str) -> Option<String> {
    read_trimmed(uevent)?.lines().find_map(|line| {
        line.strip_prefix(field)?
            .strip_prefix('=')
            .map(str::to_owned)
    })
}
