use std::{
    cmp::Ordering,
    collections::HashMap,
    future::ready,
    time::{Duration, SystemTime},
};

use futures::{Stream, StreamExt, stream};
use sottomano_toml_config::{
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
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::{Availability, AvailabilityStreams},
    blocking_background_work::{Panicked, blocking_poll},
    cpu_memory_compute::temperature_thresholds::{TemperatureThresholds, ThresholdBasis, celsius},
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::sysfs_attribute_reads::{FileReadError, list_dir, read_trimmed},
};

const WARNING_MARGIN_CELSIUS: f64 = 20.0;
const CRITICAL_MARGIN_CELSIUS: f64 = 5.0;
const UNREPORTED_WARNING_CELSIUS: f64 = 70.0;
const UNREPORTED_CRITICAL_CELSIUS: f64 = 85.0;
const CPU_PACKAGE_CHIPS: [&str; 3] = ["coretemp", "k10temp", "zenpower"];
const SOC_THERMAL_ZONE_MARKERS: [&str; 2] = ["cpu", "soc"];
const PACKAGE_FALLBACK_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

const TEMPERATURE_POLLING: PeriodicPolling = PeriodicPolling::every(Duration::from_secs(5));

#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureSample {
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
    pub samples: Vec<TemperatureSample>,
    pub throttle_limit: Option<ThermodynamicTemperature>,
    pub overrides: ThresholdOverrides,
    pub critical_since: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuCoreTemperatures {
    pub cores: Vec<CoreTemperature>,
    pub topology: CpuTopology,
}

impl SnapshotSource for CpuCoreTemperatures {
    type Error = FileReadError;
    type StreamInputs = ThresholdOverrides;

    const NAME: &'static str = "Chiplets";

    fn delivery_stream(
        overrides: ThresholdOverrides,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        TEMPERATURE_POLLING.blocking_poll_deliveries_after_setup(
            blocking_poll(move || {
                Ok(CoreSensorLayout {
                    overrides,
                    topology: CpuTopology::detect(),
                })
            }),
            |layout: CoreSensorLayout| {
                Ok(CpuCoreTemperatures {
                    cores: core_temperatures(layout.overrides),
                    topology: layout.topology,
                })
            },
        )
    }
}

#[derive(Debug, Clone)]
pub struct CoreSensorLayout {
    overrides: ThresholdOverrides,
    topology: CpuTopology,
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

    fn temperatures(&self) -> impl Iterator<Item = ThermodynamicTemperature> + '_ {
        self.samples.iter().map(|sample| sample.temperature)
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
        ThermodynamicTemperature::new::<kelvin>(total_kelvin / self.samples.len() as f64)
    }

    pub fn lowest(&self) -> Option<ThermodynamicTemperature> {
        self.temperatures().reduce(ThermodynamicTemperature::min)
    }

    #[must_use]
    pub fn hottest_sample(&self) -> Option<&TemperatureSample> {
        self.samples.iter().max_by(|a, b| {
            a.temperature
                .partial_cmp(&b.temperature)
                .unwrap_or(Ordering::Equal)
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CpuTemperatureError {
    #[error("no matching temperature sensor in /sys/class/hwmon")]
    NoTemperatureSensor,
    #[error("sensor read panicked")]
    Panicked,
}

impl From<Panicked> for CpuTemperatureError {
    fn from(Panicked: Panicked) -> Self {
        CpuTemperatureError::Panicked
    }
}

impl SnapshotSource for Temperature {
    type Error = CpuTemperatureError;
    type StreamInputs = SensorConfig;

    const NAME: &'static str = "CPU temperature";

    fn delivery_stream(config: SensorConfig) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let choice = config.sensor.clone();
        let exposed = async move {
            match spawn_blocking(move || cpu_sensors(&choice)).await {
                Ok(sensors) if !sensors.is_empty() => Availability::Available,
                Ok(_) | Err(_) => Availability::Unavailable,
            }
        };
        holding_earliest_critical(stream::once(exposed).while_available(
            move || {
                let config = config.clone();
                TEMPERATURE_POLLING.blocking_poll_deliveries_after_setup(
                    blocking_poll(move || {
                        Ok(TemperatureProbe {
                            sensors: cpu_sensors(&config.sensor),
                            config,
                        })
                    }),
                    |probe| read_temperature(&probe),
                )
            },
            || SnapshotDelivery::SourceUnavailable,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct TemperatureProbe {
    sensors: Vec<ChosenSensors>,
    config: SensorConfig,
}

fn holding_earliest_critical(
    deliveries: impl Stream<Item = SnapshotDelivery<Temperature>>,
) -> impl Stream<Item = SnapshotDelivery<Temperature>> {
    deliveries.scan(None, |held, delivery| {
        let delivery = match delivery {
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Ok(sample),
                delivery_cadence,
            } => {
                *held = sample.critical_since.map(|now| held.unwrap_or(now));
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(Temperature {
                        critical_since: *held,
                        ..sample
                    }),
                    delivery_cadence,
                }
            }
            SnapshotDelivery::SnapshotUnchanged => SnapshotDelivery::SnapshotUnchanged,
            interrupted => {
                *held = None;
                interrupted
            }
        };
        ready(Some(delivery))
    })
}

fn read_temperature(probe: &TemperatureProbe) -> Result<Temperature, CpuTemperatureError> {
    let samples: Vec<TemperatureSample> = probe
        .sensors
        .iter()
        .flat_map(ChosenSensors::samples)
        .collect();
    if samples.is_empty() {
        return Err(CpuTemperatureError::NoTemperatureSensor);
    }
    let temperature = Temperature {
        throttle_limit: throttle_limit(&samples),
        samples,
        overrides: probe.config.overrides(),
        critical_since: None,
    };
    Ok(Temperature {
        critical_since: (temperature.thresholds().level(temperature.hottest())
            == Severity::Critical)
            .then(SystemTime::now),
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
        .filter(|chip| CpuChipRole::from(*chip) == CpuChipRole::CpuPackage)
        .map(package_chip_sensors)
        .filter(|chosen| !chosen.sensors.is_empty())
        .collect();
    if !package_sensors.is_empty() {
        return package_sensors;
    }
    chips
        .into_iter()
        .filter(|chip| CpuChipRole::from(chip) == CpuChipRole::SocThermalZone)
        .map(|chip| ChosenSensors {
            sensors: chip.temperature_sensors(),
            chip,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CpuChipRole {
    CpuPackage,
    SocThermalZone,
    UnrelatedChip,
}

impl From<&HwmonChip> for CpuChipRole {
    fn from(chip: &HwmonChip) -> Self {
        if CPU_PACKAGE_CHIPS
            .iter()
            .any(|prefix| chip.name.starts_with(prefix))
        {
            CpuChipRole::CpuPackage
        } else if SOC_THERMAL_ZONE_MARKERS
            .iter()
            .any(|marker| chip.name.contains(marker))
        {
            CpuChipRole::SocThermalZone
        } else {
            CpuChipRole::UnrelatedChip
        }
    }
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
    fn samples(&self) -> Vec<TemperatureSample> {
        self.sensors
            .iter()
            .filter_map(|sensor| {
                Some(TemperatureSample {
                    chip: self.chip.name.clone(),
                    label: sensor.label.clone(),
                    temperature: self.chip.temperature(sensor)?,
                    limit: self.chip.limit(sensor, LimitKind::Crit),
                })
            })
            .collect()
    }
}

fn throttle_limit(samples: &[TemperatureSample]) -> Option<ThermodynamicTemperature> {
    samples
        .iter()
        .filter_map(|sample| sample.limit)
        .reduce(ThermodynamicTemperature::min)
}

fn core_temperatures(overrides: ThresholdOverrides) -> Vec<CoreTemperature> {
    let by_physical_core = physical_core_temperatures();
    let mut cores: Vec<CoreTemperature> = list_dir(CPU_FREQ)
        .iter()
        .filter_map(|dir| {
            let core = cpu_core_index(dir.file_name()?.to_str()?)?;
            let physical_core: u32 = read_trimmed(dir.join("topology/core_id"))?
                .parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "cpu temperature: invalid core id of {}: {parse_failure}",
                        dir.display()
                    );
                })
                .ok()?;
            let sample = by_physical_core.get(&physical_core)?;
            Some(CoreTemperature {
                core,
                temperature: sample.temperature,
                limit: sample.limit,
                overrides,
            })
        })
        .collect();
    cores.sort_by_key(|core| core.core);
    cores
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SensorSample {
    temperature: ThermodynamicTemperature,
    limit: Option<ThermodynamicTemperature>,
}

fn physical_core_temperatures() -> HashMap<u32, SensorSample> {
    HwmonChip::all()
        .into_iter()
        .filter(|chip| chip.name.starts_with("coretemp"))
        .flat_map(|chip| {
            chip.temperature_sensors()
                .into_iter()
                .filter_map(move |sensor| {
                    let physical_core = sensor
                        .label
                        .strip_prefix("Core ")?
                        .parse()
                        .inspect_err(|parse_failure| {
                            tracing::warn!(
                                "cpu temperature: invalid core number in sensor label {:?}: \
                                 {parse_failure}",
                                sensor.label
                            );
                        })
                        .ok()?;
                    let sample = SensorSample {
                        temperature: chip.temperature(&sensor)?,
                        limit: chip.limit(&sensor, LimitKind::Crit),
                    };
                    Some((physical_core, sample))
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;
    use crate::delivery_cadence_drivers::DeliveryCadence;

    fn sample_critical_since(critical_since: Option<SystemTime>) -> Temperature {
        Temperature {
            samples: Vec::new(),
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
        let deliveries =
            stream::iter([Some(first), Some(second), None, Some(third)].map(|since| {
                SnapshotDelivery::fixture_delivered(
                    Ok(sample_critical_since(since)),
                    DeliveryCadence::PeriodicPolling(TEMPERATURE_POLLING),
                )
            }));
        let held: Vec<Option<SystemTime>> = block_on(
            holding_earliest_critical(deliveries)
                .map(|delivery| match delivery {
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Ok(sample),
                        ..
                    } => sample.critical_since,
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Err(_),
                        ..
                    }
                    | SnapshotDelivery::SourceUnavailable
                    | SnapshotDelivery::InternetUnreachable(_)
                    | SnapshotDelivery::SnapshotUnchanged => None,
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
