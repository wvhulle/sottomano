use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
};

const DEFAULT_BTRFS_ALGORITHM: &str = "zlib";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Compression {
    Disabled,
    Enabled {
        algorithm: String,
        level: CompressionLevel,
        enforcement: CompressionEnforcement,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionEnforcement {
    Forced,
    Heuristic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionLevel {
    Default,
    Explicit(u8),
}

impl From<&HashMap<String, Option<String>>> for Compression {
    fn from(options: &HashMap<String, Option<String>>) -> Self {
        let setting = |key: &str| {
            options
                .get(key)
                .map(|value| value.as_deref().unwrap_or(DEFAULT_BTRFS_ALGORITHM))
        };
        if let Some(forced) = setting("compress-force") {
            return Self::parsed_mount_setting(forced, CompressionEnforcement::Forced);
        }
        setting("compress").map_or(Compression::Disabled, |setting| {
            Self::parsed_mount_setting(setting, CompressionEnforcement::Heuristic)
        })
    }
}

impl Compression {
    fn parsed_mount_setting(setting: &str, enforcement: CompressionEnforcement) -> Self {
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
            enforcement,
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
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Compression::Disabled => write!(formatter, "off"),
            Compression::Enabled {
                algorithm,
                level,
                enforcement,
            } => {
                write!(formatter, "{algorithm}{level}")?;
                if *enforcement == CompressionEnforcement::Forced {
                    write!(formatter, " (forced)")?;
                }
                Ok(())
            }
        }
    }
}

impl Display for CompressionLevel {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            CompressionLevel::Default => Ok(()),
            CompressionLevel::Explicit(level) => write!(formatter, ":{level}"),
        }
    }
}
