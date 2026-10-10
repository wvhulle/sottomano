use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    iter::successors,
    path::{Path, PathBuf},
};

use procfs::process::Process;

use super::Error;
use crate::data::source::sysfs_sensors::list_dir;

const MOUNTINFO: &str = "/proc/self/mountinfo";
const SYS_CLASS_BLOCK: &str = "/sys/class/block";
const DEFAULT_BTRFS_ALGORITHM: &str = "zlib";

#[derive(Debug, Clone, PartialEq)]
pub struct RootMount {
    pub fs_type: String,
    source: String,
    options: HashMap<String, Option<String>>,
}

impl RootMount {
    pub fn current() -> Result<Self, Error> {
        let unreadable = || Error::Unreadable(MOUNTINFO.to_owned());
        let mounts = Process::myself()
            .and_then(|process| process.mountinfo())
            .map_err(|_| unreadable())?;
        let root = mounts
            .into_iter()
            .rfind(|mount| mount.mount_point == Path::new("/"))
            .ok_or_else(unreadable)?;
        Ok(RootMount {
            fs_type: root.fs_type,
            source: root.mount_source.ok_or_else(unreadable)?,
            options: root.super_options,
        })
    }

    pub fn device(&self) -> Result<KernelDevice, Error> {
        KernelDevice::backing(&self.source)
    }

    pub fn compression(&self) -> Compression {
        Compression::from_options(&self.options)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelDevice {
    pub name: String,
}

impl KernelDevice {
    fn backing(mount_source: &str) -> Result<Self, Error> {
        let not_a_device = || Error::NotABlockDevice(mount_source.to_owned());
        let node = fs::canonicalize(mount_source).map_err(|_| not_a_device())?;
        Ok(KernelDevice {
            name: file_name(&node).ok_or_else(not_a_device)?,
        })
    }

    pub fn sysfs_dir(&self) -> PathBuf {
        Path::new(SYS_CLASS_BLOCK).join(&self.name)
    }

    pub fn whole_disk(&self) -> Self {
        let lowest = successors(Some(self.clone()), Self::first_slave)
            .last()
            .unwrap_or_else(|| self.clone());
        lowest.partition_parent().unwrap_or(lowest)
    }

    fn first_slave(&self) -> Option<Self> {
        let mut slaves = list_dir(self.sysfs_dir().join("slaves"));
        slaves.sort();
        Some(KernelDevice {
            name: file_name(slaves.first()?)?,
        })
    }

    fn partition_parent(&self) -> Option<Self> {
        let dir = self.sysfs_dir();
        if !dir.join("partition").exists() {
            return None;
        }
        let resolved = fs::canonicalize(dir).ok()?;
        Some(KernelDevice {
            name: file_name(resolved.parent()?)?,
        })
    }
}

fn file_name(path: &Path) -> Option<String> {
    Some(path.file_name()?.to_str()?.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compression {
    Disabled,
    Enabled {
        algorithm: String,
        level: CompressionLevel,
        forced: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    Default,
    Explicit(u8),
}

impl Compression {
    fn from_options(options: &HashMap<String, Option<String>>) -> Self {
        let setting = |key: &str| {
            options
                .get(key)
                .map(|value| value.as_deref().unwrap_or(DEFAULT_BTRFS_ALGORITHM))
        };
        if let Some(forced) = setting("compress-force") {
            return Self::from_setting(forced, true);
        }
        setting("compress").map_or(Compression::Disabled, |setting| {
            Self::from_setting(setting, false)
        })
    }

    fn from_setting(setting: &str, forced: bool) -> Self {
        let (algorithm, level) = setting.split_once(':').map_or(
            (setting, CompressionLevel::Default),
            |(algorithm, level)| (algorithm, CompressionLevel::parse(level)),
        );
        if algorithm == "no" {
            return Compression::Disabled;
        }
        Compression::Enabled {
            algorithm: algorithm.to_owned(),
            level,
            forced,
        }
    }
}

impl CompressionLevel {
    fn parse(level: &str) -> Self {
        level
            .parse()
            .map_or(CompressionLevel::Default, CompressionLevel::Explicit)
    }
}

impl Display for Compression {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Compression::Disabled => write!(f, "off"),
            Compression::Enabled {
                algorithm,
                level,
                forced,
            } => {
                write!(f, "{algorithm}{level}")?;
                if *forced {
                    write!(f, " (forced)")?;
                }
                Ok(())
            }
        }
    }
}

impl Display for CompressionLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            CompressionLevel::Default => Ok(()),
            CompressionLevel::Explicit(level) => write!(f, ":{level}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compression_of(options: &[(&str, Option<&str>)]) -> String {
        let options = options
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.map(str::to_owned)))
            .collect();
        Compression::from_options(&options).to_string()
    }

    #[test]
    fn algorithm_with_level() {
        assert_eq!(compression_of(&[("compress", Some("zstd:3"))]), "zstd:3");
    }

    #[test]
    fn forced_compression_is_marked() {
        assert_eq!(
            compression_of(&[("compress-force", Some("lzo"))]),
            "lzo (forced)"
        );
    }

    #[test]
    fn bare_compress_uses_the_btrfs_default() {
        assert_eq!(compression_of(&[("compress", None)]), "zlib");
    }

    #[test]
    fn absent_or_no_means_off() {
        assert_eq!(compression_of(&[("ssd", None)]), "off");
        assert_eq!(compression_of(&[("compress", Some("no"))]), "off");
    }
}
