use std::{
    cmp::Ordering,
    collections::HashMap,
    future::ready,
    time::{Duration, SystemTime},
};

use futures::{Stream, StreamExt, stream};
use sottomano_toml_config::{
    panel_menus::hardware_menu::temperature_sensors::{
        SensorChoice, SensorConfig, ThresholdOverrides,
    },
    severity::Severity,
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

mod cpu_package_sensors;

use cpu_package_sensors::detected_cpu_sensors;

const TEMPERATURE_POLLING: PeriodicPolling = PeriodicPolling::every(Duration::from_secs(5));

#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureSample {
    pub chip: String,
    pub label: String,
    pub temperature: ThermodynamicTemperature,
    pub limit: ThrottleLimit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThrottleLimit {
    Unreported,
    ThrottlesAt(ThermodynamicTemperature),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalOnset {
    BelowCritical,
    CriticalSince(SystemTime),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreTemperature {
    pub core: u32,
    pub temperature: ThermodynamicTemperature,
    pub limit: ThrottleLimit,
    pub overrides: ThresholdOverrides,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub samples: Vec<TemperatureSample>,
    pub throttle_limit: ThrottleLimit,
    pub overrides: ThresholdOverrides,
    pub critical_since: CriticalOnset,
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
    limit: ThrottleLimit,
    overrides: ThresholdOverrides,
) -> TemperatureThresholds {
    let from_hardware = match limit {
        ThrottleLimit::ThrottlesAt(throttle) => TemperatureThresholds {
            warning: throttle - margin(WARNING_MARGIN_CELSIUS),
            critical: throttle - margin(CRITICAL_MARGIN_CELSIUS),
            basis: ThresholdBasis::BelowThrottle(throttle),
        },
        ThrottleLimit::Unreported => TemperatureThresholds {
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
        self.samples.iter().max_by(|earlier_sample, later_sample| {
            earlier_sample
                .temperature
                .partial_cmp(&later_sample.temperature)
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
    deliveries.scan(CriticalOnset::BelowCritical, |held, delivery| {
        let delivery = match delivery {
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Ok(sample),
                delivery_cadence,
            } => {
                *held = match sample.critical_since {
                    CriticalOnset::BelowCritical => CriticalOnset::BelowCritical,
                    CriticalOnset::CriticalSince(now) => match *held {
                        earliest @ CriticalOnset::CriticalSince(_) => earliest,
                        CriticalOnset::BelowCritical => CriticalOnset::CriticalSince(now),
                    },
                };
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
                *held = CriticalOnset::BelowCritical;
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
        critical_since: CriticalOnset::BelowCritical,
    };
    let critical_since =
        if temperature.thresholds().level(temperature.hottest()) == Severity::Critical {
            CriticalOnset::CriticalSince(SystemTime::now())
        } else {
            CriticalOnset::BelowCritical
        };
    Ok(Temperature {
        critical_since,
        ..temperature
    })
}

fn cpu_sensors(choice: &SensorChoice) -> Vec<ChosenSensors> {
    match choice {
        SensorChoice::Automatic => detected_cpu_sensors(),
        SensorChoice::Chip { .. } => HwmonChip::chosen(choice),
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
                    limit: critical_throttle_limit(&self.chip, sensor),
                })
            })
            .collect()
    }
}

fn critical_throttle_limit(chip: &HwmonChip, sensor: &TemperatureSensor) -> ThrottleLimit {
    chip.limit(sensor, LimitKind::Crit)
        .map_or(ThrottleLimit::Unreported, ThrottleLimit::ThrottlesAt)
}

fn throttle_limit(samples: &[TemperatureSample]) -> ThrottleLimit {
    samples
        .iter()
        .filter_map(|sample| match sample.limit {
            ThrottleLimit::ThrottlesAt(limit) => Some(limit),
            ThrottleLimit::Unreported => None,
        })
        .reduce(ThermodynamicTemperature::min)
        .map_or(ThrottleLimit::Unreported, ThrottleLimit::ThrottlesAt)
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
    limit: ThrottleLimit,
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
                        limit: critical_throttle_limit(&chip, &sensor),
                    };
                    Some((physical_core, sample))
                })
        })
        .collect()
}

#[cfg(test)]
mod cpu_temperature_tests;
