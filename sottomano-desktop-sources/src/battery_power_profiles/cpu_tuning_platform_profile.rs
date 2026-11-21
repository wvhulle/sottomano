use std::path::Path;

use futures::{Stream, StreamExt, TryStreamExt};

use crate::{
    SnapshotDelivery, SnapshotSource,
    battery_power_profiles::power_profiles_daemon::active_profile_changes,
    blocking_background_work::blocking_poll,
    cpu_memory_compute::CPU_FREQ,
    delivery_cadence_drivers::{ChangeTriggerStreams, ChangeWatchFailure, SnapshotDeliveryStreams},
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses,
        sysfs_attribute_reads::{FileReadError, read_field, read_trimmed},
    },
};

const PLATFORM_PROFILE: &str = "/sys/firmware/acpi";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelSetting {
    Exposed(String),
    NotExposed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoostSetting {
    On,
    Off,
    NotExposed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuTuning {
    pub driver: String,
    pub driver_mode: KernelSetting,
    pub governor: KernelSetting,
    pub energy_preference: KernelSetting,
    pub available_energy_preferences: Vec<String>,
    pub boost: BoostSetting,
    pub platform_profile: KernelSetting,
    pub platform_profile_choices: Vec<String>,
}

fn read_setting(path: impl AsRef<Path>) -> KernelSetting {
    read_trimmed(path).map_or(KernelSetting::NotExposed, KernelSetting::Exposed)
}

fn read_boost(path: impl AsRef<Path>) -> BoostSetting {
    match read_trimmed(path).as_deref() {
        Some("1") => BoostSetting::On,
        Some(_) => BoostSetting::Off,
        None => BoostSetting::NotExposed,
    }
}

fn read_words(path: impl AsRef<Path>) -> Vec<String> {
    read_trimmed(path)
        .map(|words| words.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default()
}

fn read_cpu_tuning() -> Result<CpuTuning, FileReadError> {
    let cpu = Path::new(CPU_FREQ);
    let policy = cpu.join("cpu0/cpufreq");
    Ok(CpuTuning {
        driver: read_field(&policy.join("scaling_driver"))?,
        driver_mode: ["amd_pstate/status", "intel_pstate/status"]
            .into_iter()
            .find_map(|status| read_trimmed(cpu.join(status)))
            .map_or(KernelSetting::NotExposed, KernelSetting::Exposed),
        governor: read_setting(policy.join("scaling_governor")),
        energy_preference: read_setting(policy.join("energy_performance_preference")),
        available_energy_preferences: read_words(
            policy.join("energy_performance_available_preferences"),
        ),
        boost: read_boost(cpu.join("cpufreq/boost")),
        platform_profile: read_setting(Path::new(PLATFORM_PROFILE).join("platform_profile")),
        platform_profile_choices: read_words(
            Path::new(PLATFORM_PROFILE).join("platform_profile_choices"),
        ),
    })
}

impl SnapshotSource for CpuTuning {
    type Error = FileReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "CPU tuning";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        active_profile_changes(buses.system)
            .map_err(ChangeWatchFailure::from)
            .triggered_rereads(|| blocking_poll(read_cpu_tuning))
            .map(SnapshotDelivery::live_push)
            .repeats_as_unchanged()
    }
}
