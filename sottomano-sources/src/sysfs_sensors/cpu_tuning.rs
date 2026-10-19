use std::{future::ready, path::Path};

use futures::Stream;

use super::{CPU_FREQ, Error, read_field, read_trimmed};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, blocking_poll,
    power_profile::active_profile_changes, pushed_delivery_stream,
    stream_operators::read_once_then_per_burst,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

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

impl ReadingSource for CpuTuning {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "cpu-tuning";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        pushed_delivery_stream(ready(Ok(read_once_then_per_burst(
            active_profile_changes(buses.system),
            || blocking_poll(read_cpu_tuning),
        ))))
    }
}
