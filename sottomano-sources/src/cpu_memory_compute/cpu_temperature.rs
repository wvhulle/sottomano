use std::{
    cmp::Ordering,
    collections::HashMap,
    future::ready,
    time::{Duration, SystemTime},
};

use futures::{Stream, StreamExt, stream};
use sottomano_config::{
    severity::Severity,
    widget_groups::temperature_sensors::{SensorChoice, SensorConfig, ThresholdOverrides},
};
use tokio::task::spawn_blocking;
use uom::si::{
    f64::{TemperatureInterval, ThermodynamicTemperature},
    temperature_interval,
    thermodynamic_temperature::kelvin,
};

use super::{
    CPU_FREQ, cpu_core_index,
    cpu_topology::CpuTopology,
    hwmon_temperature_sensors::{ChosenSensors, HwmonChip, LimitKind, TemperatureSensor},
};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    blocking_background_work::blocking_poll,
    cpu_memory_compute::temperature_thresholds::{TemperatureThresholds, ThresholdBasis, celsius},
    polled_delivery_after_setup,
    stream_operators::StreamOperators,
    system_protocol_clients::sysfs_attribute_reads::{Error, list_dir, read_trimmed},
};

const WARNING_MARGIN_CELSIUS: f64 = 20.0;
const CRITICAL_MARGIN_CELSIUS: f64 = 5.0;
const UNREPORTED_WARNING_CELSIUS: f64 = 70.0;
const UNREPORTED_CRITICAL_CELSIUS: f64 = 85.0;
const CPU_PACKAGE_CHIPS: [&str; 3] = ["coretemp", "k10temp", "zenpower"];
const SOC_THERMAL_ZONE_MARKERS: [&str; 2] = ["cpu", "soc"];
const PACKAGE_FALLBACK_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

#[derive(Debug, Clone, PartialEq)]
pub struct TempReading {
    pub chip: String,
    pub label: String,
    pub temperature: ThermodynamicTemperature,
    pub limit: Option<ThermodynamicTemperature>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreTemperature {
    pub core: u32,
    pub temperature: ThermodynamicTemperature,
    pub limit: Option<ThermodynamicTemperature>,
    pub overrides: ThresholdOverrides,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub readings: Vec<TempReading>,
    pub throttle_limit: Option<ThermodynamicTemperature>,
    pub overrides: ThresholdOverrides,
    pub critical_since: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuCoreTemperatures {
    pub cores: Vec<CoreTemperature>,
    pub topology: CpuTopology,
}

impl ReadingSource for CpuCoreTemperatures {
    type Error = Error;
    type StreamInputs = ThresholdOverrides;

    const NAME: &'static str = "cpu core temperatures";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(
        overrides: ThresholdOverrides,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_after_setup(
            blocking_poll(move || {
                Ok(CoreSensorLayout {
                    overrides,
                    topology: CpuTopology::detect(),
                })
            }),
            Self::poll,
        )
    }
}

#[derive(Debug, Clone)]
pub struct CoreSensorLayout {
    overrides: ThresholdOverrides,
    topology: CpuTopology,
}

impl CpuCoreTemperatures {
    async fn poll(layout: CoreSensorLayout) -> Result<Self, Error> {
        blocking_poll(move || {
            Ok(CpuCoreTemperatures {
                cores: core_temperatures(layout.overrides),
                topology: layout.topology,
            })
        })
        .await
    }
}

fn margin(degrees: f64) -> TemperatureInterval {
    TemperatureInterval::new::<temperature_interval::degree_celsius>(degrees)
}

#[must_use]
pub fn cpu_thresholds(
    limit: Option<ThermodynamicTemperature>,
    overrides: ThresholdOverrides,
) -> TemperatureThresholds {
    let from_hardware = match limit {
        Some(throttle) => TemperatureThresholds {
            warning: throttle - margin(WARNING_MARGIN_CELSIUS),
            critical: throttle - margin(CRITICAL_MARGIN_CELSIUS),
            basis: ThresholdBasis::BelowThrottle(throttle),
        },
        None => TemperatureThresholds {
            warning: celsius(UNREPORTED_WARNING_CELSIUS),
            critical: celsius(UNREPORTED_CRITICAL_CELSIUS),
            basis: ThresholdBasis::Assumed,
        },
    };
    from_hardware.overridden_by(overrides)
}

impl Temperature {
    #[must_use]
    pub fn thresholds(&self) -> TemperatureThresholds {
        cpu_thresholds(self.throttle_limit, self.overrides)
    }

    #[must_use]
    pub fn critical(&self) -> bool {
        self.thresholds().level(self.hottest()) == Severity::Critical
    }

    fn temperatures(&self) -> impl Iterator<Item = ThermodynamicTemperature> + '_ {
        self.readings.iter().map(|reading| reading.temperature)
    }

    pub fn hottest(&self) -> ThermodynamicTemperature {
        self.temperatures()
            .fold(self.average(), ThermodynamicTemperature::max)
    }

    #[must_use]
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

    #[must_use]
    pub fn hottest_reading(&self) -> Option<&TempReading> {
        self.readings.iter().max_by(|a, b| {
            a.temperature
                .partial_cmp(&b.temperature)
                .unwrap_or(Ordering::Equal)
        })
    }
}

impl ReadingSource for Temperature {
    type Error = Error;
    type StreamInputs = SensorConfig;

    const NAME: &'static str = "temperature";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(config: SensorConfig) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let choice = config.sensor.clone();
        let exposed = async move {
            spawn_blocking(move || cpu_sensors(&choice))
                .await
                .is_ok_and(|sensors| !sensors.is_empty())
        };
        holding_earliest_critical(stream::once(exposed).while_held(
            move || {
                let config = config.clone();
                polled_delivery_after_setup(
                    blocking_poll(move || {
                        Ok(TemperatureProbe {
                            sensors: cpu_sensors(&config.sensor),
                            config,
                        })
                    }),
                    Self::poll,
                )
            },
            SourceDelivery::Absent,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct TemperatureProbe {
    sensors: Vec<ChosenSensors>,
    config: SensorConfig,
}

impl Temperature {
    async fn poll(probe: TemperatureProbe) -> Result<Self, Error> {
        blocking_poll(move || read_temperature(&probe)).await
    }
}

fn holding_earliest_critical(
    deliveries: impl Stream<Item = SourceDelivery<Temperature>>,
) -> impl Stream<Item = SourceDelivery<Temperature>> {
    deliveries.scan(None, |held, delivery| {
        let delivery = match delivery {
            SourceDelivery::Reading(Ok(reading)) => {
                *held = reading.critical_since.map(|now| held.unwrap_or(now));
                SourceDelivery::Reading(Ok(Temperature {
                    critical_since: *held,
                    ..reading
                }))
            }
            SourceDelivery::Unchanged => SourceDelivery::Unchanged,
            interrupted => {
                *held = None;
                interrupted
            }
        };
        ready(Some(delivery))
    })
}

fn read_temperature(probe: &TemperatureProbe) -> Result<Temperature, Error> {
    let readings: Vec<TempReading> = probe
        .sensors
        .iter()
        .flat_map(ChosenSensors::readings)
        .collect();
    if readings.is_empty() {
        return Err(Error::NoTemperatureSensor);
    }
    let temperature = Temperature {
        throttle_limit: throttle_limit(&readings),
        readings,
        overrides: probe.config.overrides(),
        critical_since: None,
    };
    Ok(Temperature {
        critical_since: temperature.critical().then(SystemTime::now),
        ..temperature
    })
}

fn cpu_sensors(choice: &SensorChoice) -> Vec<ChosenSensors> {
    match choice {
        SensorChoice::Automatic => detected_cpu_sensors(),
        SensorChoice::Chip { .. } => HwmonChip::chosen(choice),
    }
}

fn detected_cpu_sensors() -> Vec<ChosenSensors> {
    let chips = HwmonChip::all();
    let package_sensors: Vec<ChosenSensors> = chips
        .iter()
        .filter(|chip| is_cpu_package_chip(chip))
        .map(package_chip_sensors)
        .filter(|chosen| !chosen.sensors.is_empty())
        .collect();
    if !package_sensors.is_empty() {
        return package_sensors;
    }
    chips
        .into_iter()
        .filter(is_soc_thermal_zone)
        .map(|chip| ChosenSensors {
            sensors: chip.temperature_sensors(),
            chip,
        })
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

fn package_chip_sensors(chip: &HwmonChip) -> ChosenSensors {
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
}

impl ChosenSensors {
    fn readings(&self) -> Vec<TempReading> {
        self.sensors
            .iter()
            .filter_map(|sensor| {
                Some(TempReading {
                    chip: self.chip.name.clone(),
                    label: sensor.label.clone(),
                    temperature: self.chip.temperature(sensor)?,
                    limit: self.chip.limit(sensor, LimitKind::Crit),
                })
            })
            .collect()
    }
}

fn throttle_limit(readings: &[TempReading]) -> Option<ThermodynamicTemperature> {
    readings
        .iter()
        .filter_map(|reading| reading.limit)
        .reduce(ThermodynamicTemperature::min)
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
    limit: Option<ThermodynamicTemperature>,
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

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;

    fn reading_critical_since(critical_since: Option<SystemTime>) -> Temperature {
        Temperature {
            readings: Vec::new(),
            throttle_limit: None,
            overrides: ThresholdOverrides::default(),
            critical_since,
        }
    }

    #[test]
    fn holding_earliest_critical_keeps_the_first_stamp_until_a_gap() {
        let first = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let second = first + Duration::from_secs(5);
        let third = first + Duration::from_secs(15);
        let deliveries = stream::iter(
            [Some(first), Some(second), None, Some(third)]
                .map(|since| SourceDelivery::Reading(Ok(reading_critical_since(since)))),
        );
        let held: Vec<Option<SystemTime>> = block_on(
            holding_earliest_critical(deliveries)
                .map(|delivery| match delivery {
                    SourceDelivery::Reading(Ok(reading)) => reading.critical_since,
                    SourceDelivery::Reading(Err(_))
                    | SourceDelivery::Absent
                    | SourceDelivery::Unchanged => None,
                })
                .collect(),
        );
        assert_eq!(held, [Some(first), Some(first), None, Some(third)]);
    }

    #[test]
    fn reported_limit_sets_thresholds_below_throttling() {
        let thresholds = cpu_thresholds(Some(celsius(100.0)), ThresholdOverrides::default());
        assert_eq!(
            thresholds.basis,
            ThresholdBasis::BelowThrottle(celsius(100.0))
        );
        assert_eq!(thresholds.level(celsius(79.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(81.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(96.0)), Severity::Critical);
    }

    #[test]
    fn unreported_limit_falls_back_to_fixed_thresholds() {
        let thresholds = cpu_thresholds(None, ThresholdOverrides::default());
        assert_eq!(thresholds.basis, ThresholdBasis::Assumed);
        assert_eq!(thresholds.level(celsius(69.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(71.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(86.0)), Severity::Critical);
    }
}
