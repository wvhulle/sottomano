pub mod amd_x3d_cache_preference;
pub mod cpu_clock_frequency;
pub mod cpu_core_deep_idle;
pub mod cpu_core_loads;
pub mod cpu_power_draw;
pub mod cpu_temperature;
pub mod cpu_topology;
pub mod hwmon_temperature_sensors;
pub mod kernel_frequency_limit_causes;
pub mod l3_cache_traffic;
pub mod memory_module_temperatures;
pub mod memory_usage;
pub mod nvml_gpu_readings;
pub mod temperature_thresholds;

use std::path::Path;

use self::cpu_clock_frequency::{CoreFreq, FrequencyBand};
use crate::system_protocol_clients::sysfs_attribute_reads::{
    list_dir, read_kilohertz, read_trimmed,
};

pub(crate) const CPU_FREQ: &str = "/sys/devices/system/cpu";

pub(super) fn core_freqs() -> Vec<CoreFreq> {
    let mut cores: Vec<CoreFreq> = list_dir(CPU_FREQ)
        .iter()
        .filter_map(|dir| {
            let name = dir.file_name()?.to_str()?;
            let core: u32 = cpu_core_index(name)?;
            Some(CoreFreq {
                core,
                frequency: read_kilohertz(dir.join("cpufreq/cpuinfo_avg_freq"))
                    .or_else(|| read_kilohertz(dir.join("cpufreq/scaling_cur_freq")))?,
                band: core_frequency_band(dir)?,
                preference_rank: read_trimmed(dir.join("cpufreq/amd_pstate_prefcore_ranking"))
                    .and_then(|rank| rank.parse().ok()),
            })
        })
        .collect();
    cores.sort_by_key(|c| c.core);
    cores
}

fn core_frequency_band(dir: &Path) -> Option<FrequencyBand> {
    Some(FrequencyBand {
        floor: read_kilohertz(dir.join("cpufreq/scaling_min_freq"))?,
        policy_ceiling: read_kilohertz(dir.join("cpufreq/scaling_max_freq"))?,
        hardware_ceiling: read_kilohertz(dir.join("cpufreq/cpuinfo_max_freq"))?,
    })
}

pub(super) fn cpu_core_index(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("cpu")?;
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}
