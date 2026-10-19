use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use super::{CPU_FREQ, cpu_clock_frequency::CoreFreq};
use crate::system_protocol_clients::sysfs_attribute_reads::{list_dir, read_i64, read_trimmed};

const THERMAL_CLASS: &str = "/sys/class/thermal";
const PROCESSOR_COOLING_TYPE: &str = "Processor";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyLimitCause {
    BoostDisabled,
    PolicyCap,
    KernelThermalCooling { level: u64, deepest: u64 },
    CoreThermalThrottle { events: u64 },
    PackageThermalThrottle { events: u64 },
}

impl FrequencyLimitCause {
    #[must_use]
    pub fn is_thermal(self) -> bool {
        matches!(
            self,
            FrequencyLimitCause::KernelThermalCooling { .. }
                | FrequencyLimitCause::CoreThermalThrottle { .. }
                | FrequencyLimitCause::PackageThermalThrottle { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreLimitCauses {
    pub core: u32,
    pub causes: Vec<FrequencyLimitCause>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ThrottleCount {
    core: u64,
    package: u64,
}

impl ThrottleCount {
    fn read(cpu_dir: &Path) -> Option<Self> {
        let counter = |name: &str| {
            read_i64(cpu_dir.join("thermal_throttle").join(name))
                .and_then(|count| u64::try_from(count).ok())
        };
        Some(ThrottleCount {
            core: counter("core_throttle_count")?,
            package: counter("package_throttle_count").unwrap_or(0),
        })
    }

    fn since(self, earlier: ThrottleCount) -> ThrottleCount {
        ThrottleCount {
            core: self.core.saturating_sub(earlier.core),
            package: self.package.saturating_sub(earlier.package),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ThrottleBaseline {
    counts: Arc<Mutex<HashMap<u32, ThrottleCount>>>,
}

impl ThrottleBaseline {
    fn advance(&self, now: HashMap<u32, ThrottleCount>) -> HashMap<u32, ThrottleCount> {
        let mut counts = self.counts.lock().expect("throttle baseline");
        let since_previous_poll = now
            .iter()
            .filter_map(|(cpu, count)| Some((*cpu, count.since(*counts.get(cpu)?))))
            .collect();
        *counts = now;
        since_previous_poll
    }
}

#[derive(Debug, Clone, Copy)]
struct CoolingLevel {
    level: u64,
    deepest: u64,
}

fn processor_cooling_levels() -> HashMap<PathBuf, CoolingLevel> {
    list_dir(THERMAL_CLASS)
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
                read_i64(device.join(name)).and_then(|value| u64::try_from(value).ok())
            };
            Some((
                fs::canonicalize(device.join("device")).ok()?,
                CoolingLevel {
                    level: level("cur_state")?,
                    deepest: level("max_state")?,
                },
            ))
        })
        .collect()
}

fn boost_enabled(cpu_dir: &Path) -> Option<bool> {
    let enabled = |path: PathBuf| read_trimmed(path).map(|flag| flag == "1");
    enabled(cpu_dir.join("cpufreq/boost"))
        .or_else(|| enabled(Path::new(CPU_FREQ).join("cpufreq/boost")))
        .or_else(|| {
            enabled(Path::new(CPU_FREQ).join("intel_pstate/no_turbo")).map(|no_turbo| !no_turbo)
        })
}

fn cpu_dir(core: u32) -> PathBuf {
    Path::new(CPU_FREQ).join(format!("cpu{core}"))
}

pub(super) fn core_limit_causes(
    cores: &[CoreFreq],
    baseline: &ThrottleBaseline,
) -> Vec<CoreLimitCauses> {
    let cooling = processor_cooling_levels();
    let throttle_events_since_previous_poll = baseline.advance(
        cores
            .iter()
            .filter_map(|core| Some((core.core, ThrottleCount::read(&cpu_dir(core.core))?)))
            .collect(),
    );
    cores
        .iter()
        .map(|core| {
            let dir = cpu_dir(core.core);
            let kernel_cooling = fs::canonicalize(dir.join("firmware_node"))
                .ok()
                .and_then(|node| cooling.get(&node).copied())
                .filter(|cooling| cooling.level > 0)
                .map(|cooling| FrequencyLimitCause::KernelThermalCooling {
                    level: cooling.level,
                    deepest: cooling.deepest,
                });
            let throttled = throttle_events_since_previous_poll.get(&core.core).copied();
            CoreLimitCauses {
                core: core.core,
                causes: [
                    (boost_enabled(&dir) == Some(false))
                        .then_some(FrequencyLimitCause::BoostDisabled),
                    core.band
                        .is_capped()
                        .then_some(FrequencyLimitCause::PolicyCap),
                    kernel_cooling,
                    throttled.filter(|count| count.core > 0).map(|count| {
                        FrequencyLimitCause::CoreThermalThrottle { events: count.core }
                    }),
                    throttled.filter(|count| count.package > 0).map(|count| {
                        FrequencyLimitCause::PackageThermalThrottle {
                            events: count.package,
                        }
                    }),
                ]
                .into_iter()
                .flatten()
                .collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(core: u64, package: u64) -> HashMap<u32, ThrottleCount> {
        HashMap::from([(0, ThrottleCount { core, package })])
    }

    #[test]
    fn first_poll_reports_no_throttle_events() {
        assert!(ThrottleBaseline::default().advance(counts(7, 3)).is_empty());
    }

    #[test]
    fn later_polls_report_events_since_the_previous_poll() {
        let baseline = ThrottleBaseline::default();
        baseline.advance(counts(7, 3));
        assert_eq!(
            baseline.advance(counts(10, 3)).get(&0),
            Some(&ThrottleCount {
                core: 3,
                package: 0
            })
        );
    }
}
