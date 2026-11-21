use std::{
    collections::HashSet,
    fmt::{Display, Formatter, Result as FmtResult},
    fs,
    path::Path,
    str::FromStr,
};

use bytesize::ByteSize;

const CPU_DEVICES: &str = "/sys/devices/system/cpu";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CacheKind {
    Data,
    Instruction,
    Unified,
}

#[derive(Debug)]
pub struct UnknownCacheKind;

impl FromStr for CacheKind {
    type Err = UnknownCacheKind;

    fn from_str(kind: &str) -> Result<Self, UnknownCacheKind> {
        match kind {
            "Data" => Ok(CacheKind::Data),
            "Instruction" => Ok(CacheKind::Instruction),
            "Unified" => Ok(CacheKind::Unified),
            _ => Err(UnknownCacheKind),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CachePosition {
    pub level: u8,
    pub kind: CacheKind,
}

impl Display for CachePosition {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        let level = self.level;
        match self.kind {
            CacheKind::Data => write!(formatter, "L{level} data"),
            CacheKind::Instruction => write!(formatter, "L{level} instruction"),
            CacheKind::Unified => write!(formatter, "L{level}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct CacheInstance {
    pub(super) position: CachePosition,
    pub(super) size: ByteSize,
    pub(super) shared_cpus: String,
}

impl CacheInstance {
    fn read(index: &Path) -> Option<Self> {
        let text = |name: &str| {
            let attribute_path = index.join(name);
            let raw = fs::read_to_string(&attribute_path)
                .inspect_err(|read_failure| {
                    tracing::debug!(
                        "cpu caches: cannot read {}: {read_failure}",
                        attribute_path.display()
                    );
                })
                .ok()?;
            Some(raw.trim().to_owned())
        };
        let number = |name: &str| {
            let number_text = text(name)?;
            number_text
                .parse::<u64>()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "cpu caches: invalid {name} {number_text:?} in {}: {parse_failure}",
                        index.display()
                    );
                })
                .ok()
        };
        Some(CacheInstance {
            position: CachePosition {
                level: u8::try_from(number("level")?)
                    .inspect_err(|conversion_failure| {
                        tracing::warn!(
                            "cpu caches: cache level out of range in {}: {conversion_failure}",
                            index.display()
                        );
                    })
                    .ok()?,
                kind: text("type")?
                    .parse()
                    .inspect_err(|unknown_kind: &UnknownCacheKind| {
                        tracing::warn!(?unknown_kind, "cpu cache kind not recognised");
                    })
                    .ok()?,
            },
            size: ByteSize::b(
                number("ways_of_associativity")?
                    * number("number_of_sets")?
                    * number("coherency_line_size")?
                    * number("physical_line_partition")?,
            ),
            shared_cpus: text("shared_cpu_list")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheInstances {
    pub size: ByteSize,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLevel {
    pub position: CachePosition,
    pub instances: Vec<CacheInstances>,
}

impl From<&[CacheInstance]> for CacheLevel {
    fn from(instances: &[CacheInstance]) -> Self {
        CacheLevel {
            position: instances[0].position,
            instances: instances
                .chunk_by(|earlier_instance, later_instance| {
                    earlier_instance.size == later_instance.size
                })
                .map(|same_size| CacheInstances {
                    size: same_size[0].size,
                    count: same_size.len(),
                })
                .collect(),
        }
    }
}

pub(super) fn cache_levels() -> Vec<CacheLevel> {
    let instances = fs::read_dir(CPU_DEVICES)
        .inspect_err(|listing_failure| {
            tracing::warn!("cpu caches: cannot list {CPU_DEVICES}: {listing_failure}");
        })
        .into_iter()
        .flatten()
        .filter_map(|cpu_entry| {
            cpu_entry
                .inspect_err(|entry_failure| {
                    tracing::debug!(
                        "cpu caches: skipping unreadable entry of {CPU_DEVICES}: {entry_failure}"
                    );
                })
                .ok()
        })
        .filter_map(|cpu| {
            let cache_directory = cpu.path().join("cache");
            fs::read_dir(&cache_directory)
                .inspect_err(|listing_failure| {
                    tracing::debug!(
                        "cpu caches: cannot list {}: {listing_failure}",
                        cache_directory.display()
                    );
                })
                .ok()
        })
        .flatten()
        .filter_map(|index_entry| {
            index_entry
                .inspect_err(|entry_failure| {
                    tracing::debug!("cpu caches: skipping unreadable cache entry: {entry_failure}");
                })
                .ok()
        })
        .filter_map(|index| CacheInstance::read(&index.path()));
    grouped_cache_levels(instances)
}

fn grouped_cache_levels(instances: impl IntoIterator<Item = CacheInstance>) -> Vec<CacheLevel> {
    let mut distinct: Vec<CacheInstance> = instances
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    distinct.sort_by(|earlier_instance, later_instance| {
        earlier_instance
            .position
            .cmp(&later_instance.position)
            .then(later_instance.size.cmp(&earlier_instance.size))
    });
    distinct
        .chunk_by(|earlier_instance, later_instance| {
            earlier_instance.position == later_instance.position
        })
        .map(CacheLevel::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_caches_are_counted_once_and_grouped_by_size() {
        let unified = |level, size, shared_cpus: &str| CacheInstance {
            position: CachePosition {
                level,
                kind: CacheKind::Unified,
            },
            size,
            shared_cpus: shared_cpus.to_owned(),
        };
        let instances = [
            unified(2, ByteSize::mib(1), "0,16"),
            unified(2, ByteSize::mib(1), "1,17"),
            unified(3, ByteSize::mib(32), "8-15,24-31"),
            unified(2, ByteSize::mib(1), "0,16"),
            unified(3, ByteSize::mib(96), "0-7,16-23"),
            unified(3, ByteSize::mib(96), "0-7,16-23"),
        ];
        assert_eq!(
            grouped_cache_levels(instances),
            vec![
                CacheLevel {
                    position: CachePosition {
                        level: 2,
                        kind: CacheKind::Unified,
                    },
                    instances: vec![CacheInstances {
                        size: ByteSize::mib(1),
                        count: 2,
                    }],
                },
                CacheLevel {
                    position: CachePosition {
                        level: 3,
                        kind: CacheKind::Unified,
                    },
                    instances: vec![
                        CacheInstances {
                            size: ByteSize::mib(96),
                            count: 1,
                        },
                        CacheInstances {
                            size: ByteSize::mib(32),
                            count: 1,
                        },
                    ],
                },
            ]
        );
    }
}
