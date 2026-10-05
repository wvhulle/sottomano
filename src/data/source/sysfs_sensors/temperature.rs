use std::{collections::HashMap, time::Duration};

use tokio::task::spawn_blocking;
use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature::kelvin};

use super::{
    CPU_FREQ, Error, cpu_core_index,
    cpu_topology::CpuTopology,
    hwmon::{ChosenSensors, HwmonChip, LimitKind, TemperatureSensor},
    list_dir, read_trimmed,
};
use crate::{
    config::temperature::{SensorChoice, SensorConfig, ThresholdOverrides},
    data::source::Source,
};

const CPU_PACKAGE_CHIPS: [&str; 3] = ["coretemp", "k10temp", "zenpower"];
const SOC_THERMAL_ZONE_MARKERS: [&str; 2] = ["cpu", "soc"];
const PACKAGE_FALLBACK_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

#[derive(Debug, Clone, PartialEq)]
pub struct TempReading {
    pub chip: String,
    pub temperature: ThermodynamicTemperature,
    limit: ThermalLimit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreTemperature {
    pub core: u32,
    pub temperature: ThermodynamicTemperature,
    pub limit: ThermalLimit,
    pub overrides: ThresholdOverrides,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub readings: Vec<TempReading>,
    pub cores: Vec<CoreTemperature>,
    pub throttle_limit: ThermalLimit,
    pub overrides: ThresholdOverrides,
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

    type Key = SensorConfig;

    const NAME: &'static str = "temperature";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn available(config: &SensorConfig) -> bool {
        let choice = config.sensor.clone();
        spawn_blocking(move || cpu_temp_readings(&choice))
            .await
            .is_ok_and(|readings| !readings.is_empty())
    }

    async fn poll(config: SensorConfig) -> Result<Self, Self::Error> {
        spawn_blocking(move || read_temperature(&config))
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_temperature(config: &SensorConfig) -> Result<Temperature, Error> {
    let readings = cpu_temp_readings(&config.sensor);
    if readings.is_empty() {
        return Err(Error::NoTemperatureSensor);
    }
    Ok(Temperature {
        throttle_limit: throttle_limit(&readings),
        readings,
        cores: core_temperatures(config.overrides()),
        overrides: config.overrides(),
        topology: CpuTopology::current(),
    })
}

fn cpu_temp_readings(choice: &SensorChoice) -> Vec<TempReading> {
    match choice {
        SensorChoice::Automatic => detected_cpu_readings(),
        SensorChoice::Chip { .. } => HwmonChip::chosen(choice)
            .iter()
            .flat_map(ChosenSensors::readings)
            .collect(),
    }
}

fn detected_cpu_readings() -> Vec<TempReading> {
    let chips = HwmonChip::all();
    let package_readings: Vec<TempReading> = chips
        .iter()
        .filter(|chip| is_cpu_package_chip(chip))
        .flat_map(package_chip_readings)
        .collect();
    if !package_readings.is_empty() {
        return package_readings;
    }
    chips
        .into_iter()
        .filter(is_soc_thermal_zone)
        .map(|chip| ChosenSensors {
            sensors: chip.temperature_sensors(),
            chip,
        })
        .flat_map(|chosen| chosen.readings())
        .collect()
}

fn is_cpu_package_chip(chip: &HwmonChip) -> bool {
    CPU_PACKAGE_CHIPS
        .iter()
        .any(|prefix| chip.name.starts_with(prefix))
}

fn is_soc_thermal_zone(chip: &HwmonChip) -> bool {
    SOC_THERMAL_ZONE_MARKERS
        .iter()
        .any(|marker| chip.name.contains(marker))
}

fn package_chip_readings(chip: &HwmonChip) -> Vec<TempReading> {
    let sensors = chip.temperature_sensors();
    let core_prefix = if chip.name.starts_with("coretemp") {
        "Core "
    } else {
        "Tccd"
    };
    let per_core: Vec<TemperatureSensor> = sensors
        .iter()
        .filter(|sensor| sensor.label.starts_with(core_prefix))
        .cloned()
        .collect();
    let chosen = if per_core.is_empty() {
        PACKAGE_FALLBACK_LABELS
            .iter()
            .find_map(|label| sensors.iter().find(|sensor| &sensor.label == label))
            .into_iter()
            .cloned()
            .collect()
    } else {
        per_core
    };
    ChosenSensors {
        chip: chip.clone(),
        sensors: chosen,
    }
    .readings()
}

impl ChosenSensors {
    fn readings(&self) -> Vec<TempReading> {
        self.sensors
            .iter()
            .filter_map(|sensor| {
                Some(TempReading {
                    chip: self.chip.name.clone(),
                    temperature: self.chip.temperature(sensor)?,
                    limit: self.chip.limit(sensor, LimitKind::Crit),
                })
            })
            .collect()
    }
}

fn throttle_limit(readings: &[TempReading]) -> ThermalLimit {
    readings
        .iter()
        .filter_map(|reading| match reading.limit {
            ThermalLimit::Reported(limit) => Some(limit),
            ThermalLimit::Unreported => None,
        })
        .reduce(ThermodynamicTemperature::min)
        .map_or(ThermalLimit::Unreported, ThermalLimit::Reported)
}

fn core_temperatures(overrides: ThresholdOverrides) -> Vec<CoreTemperature> {
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
                overrides,
            })
        })
        .collect();
    cores.sort_by_key(|core| core.core);
    cores
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SensorReading {
    temperature: ThermodynamicTemperature,
    limit: ThermalLimit,
}

fn physical_core_temperatures() -> HashMap<u32, SensorReading> {
    HwmonChip::all()
        .into_iter()
        .filter(|chip| chip.name.starts_with("coretemp"))
        .flat_map(|chip| {
            chip.temperature_sensors()
                .into_iter()
                .filter_map(move |sensor| {
                    let physical_core = sensor.label.strip_prefix("Core ")?.parse().ok()?;
                    let reading = SensorReading {
                        temperature: chip.temperature(&sensor)?,
                        limit: chip.limit(&sensor, LimitKind::Crit),
                    };
                    Some((physical_core, reading))
                })
        })
        .collect()
}
