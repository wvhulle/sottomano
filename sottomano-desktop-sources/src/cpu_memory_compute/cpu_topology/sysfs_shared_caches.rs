use std::{fs, path::Path};

use uom::si::{
    f64::Information,
    information::{kibibyte, mebibyte},
};

use super::cpulist;
use crate::system_protocol_clients::sysfs_attribute_reads::read_trimmed;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum LastLevelCache {
    NotExposed,
    Shared(SharedCache),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct SharedCache {
    pub(super) level: u32,
    pub(super) size: Information,
    pub(super) sharing_cpus: Vec<u32>,
}

impl SharedCache {
    pub(super) fn read_last_level(cpu_directory: &Path) -> LastLevelCache {
        let Ok(cache_entries) =
            fs::read_dir(cpu_directory.join("cache")).inspect_err(|listing_failure| {
                tracing::debug!(
                    "cpu topology: cannot list caches of {}: {listing_failure}",
                    cpu_directory.display()
                );
            })
        else {
            return LastLevelCache::NotExposed;
        };
        cache_entries
            .filter_map(|cache_entry| {
                cache_entry
                    .inspect_err(|entry_failure| {
                        tracing::debug!(
                            "cpu topology: skipping unreadable cache entry of {}: {entry_failure}",
                            cpu_directory.display()
                        );
                    })
                    .ok()
            })
            .filter_map(|index| Self::read(&index.path()))
            .max_by_key(|cache| cache.level)
            .map_or(LastLevelCache::NotExposed, LastLevelCache::Shared)
    }

    fn read(index_directory: &Path) -> Option<Self> {
        Some(SharedCache {
            level: read_trimmed(index_directory.join("level"))?
                .parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "cpu topology: invalid cache level in {}: {parse_failure}",
                        index_directory.display()
                    );
                })
                .ok()?,
            size: parse_cache_size(&read_trimmed(index_directory.join("size"))?)?,
            sharing_cpus: cpulist(&read_trimmed(index_directory.join("shared_cpu_list"))?)
                .collect(),
        })
    }
}

pub(super) fn parse_cache_size(text: &str) -> Option<Information> {
    let parsed_size = if let Some(kibibytes) = text.strip_suffix('K') {
        kibibytes.parse().map(Information::new::<kibibyte>)
    } else if let Some(mebibytes) = text.strip_suffix('M') {
        mebibytes.parse().map(Information::new::<mebibyte>)
    } else {
        tracing::warn!("cpu topology: cache size {text:?} lacks a K or M unit");
        return None;
    };
    parsed_size
        .inspect_err(|parse_failure| {
            tracing::warn!("cpu topology: invalid cache size {text:?}: {parse_failure}");
        })
        .ok()
}
