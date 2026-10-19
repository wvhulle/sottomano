use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use uom::si::{f64::Frequency, frequency::kilohertz};

use crate::Panicked;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Unreadable(String),
    Unwritable { path: String, detail: String },
    Unparsable(String),
    NoTemperatureSensor,
    Panicked,
}

impl From<Panicked> for Error {
    fn from(Panicked: Panicked) -> Self {
        Error::Panicked
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Unreadable(path) => write!(f, "cannot read {path}"),
            Error::Unwritable { path, detail } => write!(f, "cannot write {path}: {detail}"),
            Error::Unparsable(path) => write!(f, "unexpected contents in {path}"),
            Error::NoTemperatureSensor => {
                write!(f, "no matching temperature sensor in /sys/class/hwmon")
            }
            Error::Panicked => write!(f, "sensor read panicked"),
        }
    }
}

pub fn read_field(path: &Path) -> Result<String, Error> {
    read_trimmed(path).ok_or_else(|| Error::Unreadable(path.display().to_string()))
}

pub fn parse_field<T: FromStr>(path: &Path) -> Result<T, Error> {
    read_field(path)?
        .parse()
        .map_err(|_| Error::Unparsable(path.display().to_string()))
}

#[must_use]
pub fn read_kilohertz(path: impl AsRef<Path>) -> Option<Frequency> {
    read_i64(path).map(|khz| Frequency::new::<kilohertz>(khz as f64))
}

#[must_use]
pub fn list_dir(dir: impl AsRef<Path>) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect()
}

#[must_use]
pub fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    Some(fs::read_to_string(path).ok()?.trim().to_string())
}

#[must_use]
pub fn read_i64(path: impl AsRef<Path>) -> Option<i64> {
    read_trimmed(path)?.parse().ok()
}
