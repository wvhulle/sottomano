use std::path::{Path, PathBuf};

use super::{
    CPU_FREQ,
    acpi_processor_cooling::ProcessorCoolingDevices,
    cpu_clock_frequency::{CoreFreq, PolicyRestriction},
    cpu_dir, intel_pstate_no_turbo,
    intel_thermal_throttle_counters::ThrottleBaseline,
};
use crate::system_protocol_clients::sysfs_attribute_reads::read_trimmed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyLimitCause {
    BoostDisabled,
    PolicyCap,
    KernelThermalCooling { level: u64, deepest: u64 },
    CoreThermalThrottle { events: u64 },
    PackageThermalThrottle { events: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreLimitCauses {
    pub core: u32,
    pub causes: Vec<FrequencyLimitCause>,
}

fn cpufreq_boost_enabled(cpu_dir: &Path) -> Option<bool> {
    let enabled = |path: PathBuf| read_trimmed(path).map(|flag| flag == "1");
    enabled(cpu_dir.join("cpufreq/boost"))
        .or_else(|| enabled(Path::new(CPU_FREQ).join("cpufreq/boost")))
        .or_else(intel_pstate_no_turbo::turbo_enabled)
}

pub(super) fn core_limit_causes(
    cores: &[CoreFreq],
    baseline: &ThrottleBaseline,
) -> Vec<CoreLimitCauses> {
    let cooling = ProcessorCoolingDevices::detect();
    let throttle_events = baseline.events_since_previous_poll(cores.iter().map(|core| core.core));
    cores
        .iter()
        .map(|core| {
            let dir = cpu_dir(core.core);
            let throttled = throttle_events.get(&core.core).copied();
            CoreLimitCauses {
                core: core.core,
                causes: [
                    (cpufreq_boost_enabled(&dir) == Some(false))
                        .then_some(FrequencyLimitCause::BoostDisabled),
                    (core.band.policy_restriction() == PolicyRestriction::Capped)
                        .then_some(FrequencyLimitCause::PolicyCap),
                    cooling.engaged_on(&dir).map(|cooling| {
                        FrequencyLimitCause::KernelThermalCooling {
                            level: cooling.level,
                            deepest: cooling.deepest,
                        }
                    }),
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
