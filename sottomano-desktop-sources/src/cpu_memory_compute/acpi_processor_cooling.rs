use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::system_protocol_clients::sysfs_attribute_reads::{list_dir, read_i64, read_trimmed};

const THERMAL_CLASS: &str = "/sys/class/thermal";
const PROCESSOR_COOLING_TYPE: &str = "Processor";

#[derive(Debug, Clone, Copy)]
pub(super) struct CoolingLevel {
    pub level: u64,
    pub deepest: u64,
}

pub(super) struct ProcessorCoolingDevices {
    by_firmware_node: HashMap<PathBuf, CoolingLevel>,
}

impl ProcessorCoolingDevices {
    pub(super) fn detect() -> Self {
        Self {
            by_firmware_node: list_dir(THERMAL_CLASS)
                .into_iter()
                .filter(|device| {
                    device
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("cooling_device"))
                })
                .filter(|device| {
                    read_trimmed(device.join("type")).as_deref() == Some(PROCESSOR_COOLING_TYPE)
                })
                .filter_map(|device| {
                    let level = |name: &str| {
                        read_i64(device.join(name)).and_then(|value| {
                            u64::try_from(value)
                                .inspect_err(|conversion_failure| {
                                    tracing::warn!(
                                        "processor cooling: negative {name} {value} in {}: \
                                         {conversion_failure}",
                                        device.display()
                                    );
                                })
                                .ok()
                        })
                    };
                    Some((
                        fs::canonicalize(device.join("device"))
                            .inspect_err(|resolve_failure| {
                                tracing::debug!(
                                    "processor cooling: cannot resolve device of {}: \
                                     {resolve_failure}",
                                    device.display()
                                );
                            })
                            .ok()?,
                        CoolingLevel {
                            level: level("cur_state")?,
                            deepest: level("max_state")?,
                        },
                    ))
                })
                .collect(),
        }
    }

    pub(super) fn engaged_on(&self, cpu_dir: &Path) -> Option<CoolingLevel> {
        fs::canonicalize(cpu_dir.join("firmware_node"))
            .inspect_err(|resolve_failure| {
                tracing::debug!(
                    "processor cooling: cannot resolve firmware node of {}: {resolve_failure}",
                    cpu_dir.display()
                );
            })
            .ok()
            .and_then(|node| self.by_firmware_node.get(&node).copied())
            .filter(|cooling| cooling.level > 0)
    }
}
