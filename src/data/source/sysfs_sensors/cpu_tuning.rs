use std::{path::Path, time::Duration};

use tokio::task::spawn_blocking;

use super::{CPU_FREQ, Error, read_field, read_trimmed};
use crate::data::source::Source;

const PLATFORM_PROFILE: &str = "/sys/firmware/acpi";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuTuning {
    pub driver: String,
    pub driver_mode: Option<String>,
    pub governor: Option<String>,
    pub energy_preference: Option<String>,
    pub available_energy_preferences: Vec<String>,
    pub boost: Option<bool>,
    pub platform_profile: Option<String>,
    pub platform_profile_choices: Vec<String>,
}

fn read_words(path: impl AsRef<Path>) -> Vec<String> {
    read_trimmed(path)
        .map(|words| words.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

fn read_cpu_tuning() -> Result<CpuTuning, Error> {
    let cpu = Path::new(CPU_FREQ);
    let policy = cpu.join("cpu0/cpufreq");
    Ok(CpuTuning {
        driver: read_field(&policy.join("scaling_driver"))?,
        driver_mode: ["amd_pstate/status", "intel_pstate/status"]
            .into_iter()
            .find_map(|status| read_trimmed(cpu.join(status))),
        governor: read_trimmed(policy.join("scaling_governor")),
        energy_preference: read_trimmed(policy.join("energy_performance_preference")),
        available_energy_preferences: read_words(
            policy.join("energy_performance_available_preferences"),
        ),
        boost: read_trimmed(cpu.join("cpufreq/boost")).map(|boost| boost == "1"),
        platform_profile: read_trimmed(Path::new(PLATFORM_PROFILE).join("platform_profile")),
        platform_profile_choices: read_words(
            Path::new(PLATFORM_PROFILE).join("platform_profile_choices"),
        ),
    })
}

impl Source for CpuTuning {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "cpu-tuning";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_cpu_tuning)
            .await
            .map_err(|_| Error::Panicked)?
    }
}
