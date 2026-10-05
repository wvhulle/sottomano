use std::{collections::HashMap, path::Path, time::Duration};

use tokio::task::spawn_blocking;
use uom::si::{
    f64::ThermodynamicTemperature,
    thermodynamic_temperature::{degree_celsius, kelvin},
};

use super::{
    CPU_FREQ, Error, cpu_core_index, cpu_topology::CpuTopology, list_dir, read_i64, read_trimmed,
};
use crate::data::source::Source;

const HWMON: &str = "/sys/class/hwmon";
const CPU_CHIP_PREFIXES: [&str; 3] = ["coretemp", "k10temp", "zenpower"];
const TEMP_FALLBACK_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

#[derive(Debug, Clone, PartialEq)]
pub struct TempReading {
    pub chip: String,
    pub temperature: ThermodynamicTemperature,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreTemperature {
    pub core: u32,
    pub temperature: ThermodynamicTemperature,
    pub limit: ThermalLimit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub readings: Vec<TempReading>,
    pub cores: Vec<CoreTemperature>,
    pub throttle_limit: ThermalLimit,
    pub topology: CpuTopology,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalLimit {
    Reported(ThermodynamicTemperature),
    Unreported,
}

impl Temperature {
    fn temperatures(&self) -> impl Iterator<Item = ThermodynamicTemperature> + '_ {
        self.readings.iter().map(|reading| reading.temperature)
    }

    pub fn hottest(&self) -> ThermodynamicTemperature {
        self.temperatures()
            .fold(self.average(), ThermodynamicTemperature::max)
    }

    pub fn average(&self) -> ThermodynamicTemperature {
        let total_kelvin: f64 = self
            .temperatures()
            .map(|temperature| temperature.get::<kelvin>())
            .sum();
        ThermodynamicTemperature::new::<kelvin>(total_kelvin / self.readings.len() as f64)
    }

    pub fn lowest(&self) -> Option<ThermodynamicTemperature> {
        self.temperatures().reduce(ThermodynamicTemperature::min)
    }
}

impl Source for Temperature {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "temperature";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn available(_key: &()) -> bool {
        spawn_blocking(cpu_temp_readings)
            .await
            .is_ok_and(|readings| !readings.is_empty())
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_temperature)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_temperature() -> Result<Temperature, Error> {
    let readings = cpu_temp_readings();
    if readings.is_empty() {
        return Err(Error::Unreadable(HWMON.to_owned()));
    }
    Ok(Temperature {
        readings,
        cores: core_temperatures(),
        throttle_limit: throttle_limit(),
        topology: CpuTopology::current(),
    })
}

fn core_temperatures() -> Vec<CoreTemperature> {
    let by_physical_core = physical_core_temperatures();
    let mut cores: Vec<CoreTemperature> = list_dir(CPU_FREQ)
        .iter()
        .filter_map(|dir| {
            let core = cpu_core_index(dir.file_name()?.to_str()?)?;
            let physical_core: u32 = read_trimmed(dir.join("topology/core_id"))?.parse().ok()?;
            let reading = by_physical_core.get(&physical_core)?;
            Some(CoreTemperature {
                core,
                temperature: reading.temperature,
                limit: reading.limit,
            })
        })
        .collect();
    cores.sort_by_key(|core| core.core);
    cores
}

fn throttle_limit() -> ThermalLimit {
    list_dir(HWMON)
        .iter()
        .filter(|chip| is_coretemp(chip))
        .flat_map(|chip| {
            temp_labels(chip)
                .into_iter()
                .filter_map(move |(index, _)| read_i64(chip.join(format!("temp{index}_crit"))))
        })
        .map(celsius_from_millidegrees)
        .reduce(ThermodynamicTemperature::min)
        .map_or(ThermalLimit::Unreported, ThermalLimit::Reported)
}

fn is_coretemp(chip: &Path) -> bool {
    read_trimmed(chip.join("name")).is_some_and(|name| name.starts_with("coretemp"))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SensorReading {
    temperature: ThermodynamicTemperature,
    limit: ThermalLimit,
}

fn physical_core_temperatures() -> HashMap<u32, SensorReading> {
    list_dir(HWMON)
        .iter()
        .filter(|chip| is_coretemp(chip))
        .flat_map(|chip| {
            temp_labels(chip)
                .into_iter()
                .filter_map(move |(index, label)| {
                    let physical_core = label.strip_prefix("Core ")?.parse().ok()?;
                    let millidegrees = read_i64(chip.join(format!("temp{index}_input")))?;
                    let reading = SensorReading {
                        temperature: celsius_from_millidegrees(millidegrees),
                        limit: sensor_limit(chip, index),
                    };
                    Some((physical_core, reading))
                })
        })
        .collect()
}

fn sensor_limit(chip: &Path, index: u32) -> ThermalLimit {
    read_i64(chip.join(format!("temp{index}_crit")))
        .map(celsius_from_millidegrees)
        .map_or(ThermalLimit::Unreported, ThermalLimit::Reported)
}

fn celsius_from_millidegrees(millidegrees: i64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(millidegrees as f64 / 1000.0)
}

fn cpu_temp_readings() -> Vec<TempReading> {
    let mut chips = list_dir(HWMON);
    chips.sort();
    chips
        .iter()
        .filter_map(|chip| {
            let name = read_trimmed(chip.join("name"))?;
            let is_coretemp = name.starts_with("coretemp");
            CPU_CHIP_PREFIXES
                .iter()
                .any(|p| name.starts_with(p))
                .then(|| chip_readings(chip, &name, is_coretemp))
        })
        .flatten()
        .collect()
}

fn chip_readings(chip: &Path, name: &str, is_coretemp: bool) -> Vec<TempReading> {
    let labels: Vec<(u32, String)> = temp_labels(chip);
    let is_core = |label: &str| {
        if is_coretemp {
            label.starts_with("Core ")
        } else {
            label.starts_with("Tccd")
        }
    };

    let indices: Vec<u32> = if labels.iter().any(|(_, l)| is_core(l)) {
        labels
            .iter()
            .filter(|(_, l)| is_core(l))
            .map(|(i, _)| *i)
            .collect()
    } else {
        TEMP_FALLBACK_LABELS
            .iter()
            .find_map(|fb| labels.iter().find(|(_, l)| l == fb).map(|(i, _)| *i))
            .into_iter()
            .collect()
    };

    indices
        .iter()
        .filter_map(|i| read_i64(chip.join(format!("temp{i}_input"))))
        .map(|millidegrees| TempReading {
            chip: name.to_string(),
            temperature: celsius_from_millidegrees(millidegrees),
        })
        .collect()
}

fn temp_labels(chip: &Path) -> Vec<(u32, String)> {
    list_dir(chip)
        .iter()
        .filter_map(|p| {
            let stem = p.file_name()?.to_str()?;
            let index: u32 = stem
                .strip_prefix("temp")?
                .strip_suffix("_label")?
                .parse()
                .ok()?;
            Some((index, read_trimmed(p)?))
        })
        .collect()
}
