use std::{path::Path, time::Duration};

use tokio::task::spawn_blocking;
use uom::si::{
    f64::ThermodynamicTemperature,
    thermodynamic_temperature::{degree_celsius, kelvin},
};

use super::{Error, list_dir, read_i64, read_trimmed};
use crate::data::source::Source;

const HWMON: &str = "/sys/class/hwmon";
const CPU_CHIP_PREFIXES: [&str; 3] = ["coretemp", "k10temp", "zenpower"];
const TEMP_FALLBACK_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

#[derive(Debug, Clone, PartialEq)]
pub struct TempReading {
    pub chip: String,
    pub temperature: ThermodynamicTemperature,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub readings: Vec<TempReading>,
}

impl Temperature {
    fn temperatures(&self) -> impl Iterator<Item = ThermodynamicTemperature> + '_ {
        self.readings.iter().map(|reading| reading.temperature)
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

    pub fn highest(&self) -> Option<ThermodynamicTemperature> {
        self.temperatures().reduce(ThermodynamicTemperature::max)
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
    Ok(Temperature { readings })
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
            temperature: ThermodynamicTemperature::new::<degree_celsius>(
                millidegrees as f64 / 1000.0,
            ),
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
