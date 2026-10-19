use std::{path::Path, time::Duration};

use futures::Stream;
use sottomano_config::{
    severity::Severity,
    widget_groups::temperature_sensors::{SensorChoice, SensorConfig, ThresholdOverrides},
};
use uom::si::f64::ThermodynamicTemperature;

use super::{
    Error,
    btrfs_filesystem_space::{BtrfsFilesystem, DeviceErrors},
    root_mount_facts::{KernelDevice, RootMount},
};
use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    blocking_background_work::blocking_poll,
    cpu_memory_compute::{
        hwmon_temperature_sensors::{ChosenSensors, HwmonChip, LimitKind},
        temperature_thresholds::{TemperatureThresholds, ThresholdBasis, celsius},
    },
    host_machine_identity::udev_hardware_identity::DriveIdentity,
    polled_delivery_stream,
};

const COMPOSITE_LABEL: &str = "Composite";
const DRIVETEMP_MODULE: &str = "/sys/module/drivetemp";
const UNREPORTED_WARNING_CELSIUS: f64 = 60.0;
const UNREPORTED_CRITICAL_CELSIUS: f64 = 70.0;

#[derive(Debug, Clone, PartialEq)]
pub struct DriveHealth {
    pub device_name: String,
    pub identity: DriveIdentity,
    pub temperature: DriveTemperature,
    pub overrides: ThresholdOverrides,
    pub errors: Option<DeviceErrors>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveTemperature {
    Reported {
        current: ThermodynamicTemperature,
        warning: Option<ThermodynamicTemperature>,
        critical: Option<ThermodynamicTemperature>,
    },
    Unreported(MissingSensor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingSensor {
    DrivetempNotLoaded,
    NotExposed,
}

impl DriveHealth {
    #[must_use]
    pub fn temperature_level(&self) -> Severity {
        match self.temperature {
            DriveTemperature::Reported { current, .. } => {
                self.temperature_thresholds().level(current)
            }
            DriveTemperature::Unreported(_) => Severity::Normal,
        }
    }

    #[must_use]
    pub fn temperature_thresholds(&self) -> TemperatureThresholds {
        TemperatureThresholds {
            warning: self
                .temperature
                .warning()
                .unwrap_or_else(|| celsius(UNREPORTED_WARNING_CELSIUS)),
            critical: self
                .temperature
                .critical()
                .unwrap_or_else(|| celsius(UNREPORTED_CRITICAL_CELSIUS)),
            basis: match self.temperature.critical() {
                Some(_) => ThresholdBasis::Reported,
                None => ThresholdBasis::Assumed,
            },
        }
        .overridden_by(self.overrides)
    }
}

impl DriveTemperature {
    #[must_use]
    pub fn current(&self) -> Option<ThermodynamicTemperature> {
        match self {
            DriveTemperature::Reported { current, .. } => Some(*current),
            DriveTemperature::Unreported(_) => None,
        }
    }

    fn warning(&self) -> Option<ThermodynamicTemperature> {
        match self {
            DriveTemperature::Reported { warning, .. } => *warning,
            DriveTemperature::Unreported(_) => None,
        }
    }

    fn critical(&self) -> Option<ThermodynamicTemperature> {
        match self {
            DriveTemperature::Reported { critical, .. } => *critical,
            DriveTemperature::Unreported(_) => None,
        }
    }
}

impl ReadingSource for DriveHealth {
    type Error = Error;
    type StreamInputs = SensorConfig;

    const NAME: &'static str = "drive_health";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(60));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl DriveHealth {
    async fn poll(config: SensorConfig) -> Result<Self, Error> {
        blocking_poll(move || read_drive_health(&config)).await
    }
}

fn read_drive_health(config: &SensorConfig) -> Result<DriveHealth, Error> {
    let mount = RootMount::current()?;
    let device = mount.device()?;
    let disk = device.whole_disk();
    let controller = disk.sysfs_dir().join("device");
    let errors = if mount.fs_type == "btrfs" {
        Some(BtrfsFilesystem::containing(&device)?.device_errors()?)
    } else {
        None
    };
    Ok(DriveHealth {
        device_name: disk.name.clone(),
        identity: DriveIdentity::of_controller(&controller),
        temperature: DriveTemperature::read(&config.sensor, &disk, &controller),
        overrides: config.overrides(),
        errors,
    })
}

impl DriveTemperature {
    fn read(choice: &SensorChoice, disk: &KernelDevice, controller: &Path) -> Self {
        let candidates = match choice {
            SensorChoice::Automatic => HwmonChip::of_device(controller)
                .into_iter()
                .map(|chip| ChosenSensors {
                    sensors: chip.temperature_sensors(),
                    chip,
                })
                .collect(),
            SensorChoice::Chip { .. } => HwmonChip::chosen(choice),
        };
        candidates
            .iter()
            .find_map(Self::of_chosen)
            .unwrap_or_else(|| DriveTemperature::Unreported(MissingSensor::for_disk(disk)))
    }

    fn of_chosen(chosen: &ChosenSensors) -> Option<Self> {
        let sensor = chosen
            .sensors
            .iter()
            .find(|sensor| sensor.label == COMPOSITE_LABEL)
            .or_else(|| chosen.sensors.first())?;
        Some(DriveTemperature::Reported {
            current: chosen.chip.temperature(sensor)?,
            warning: chosen.chip.limit(sensor, LimitKind::Max),
            critical: chosen.chip.limit(sensor, LimitKind::Crit),
        })
    }
}

impl MissingSensor {
    fn for_disk(disk: &KernelDevice) -> Self {
        let is_scsi_disk = disk.name.starts_with("sd");
        if is_scsi_disk && !Path::new(DRIVETEMP_MODULE).exists() {
            MissingSensor::DrivetempNotLoaded
        } else {
            MissingSensor::NotExposed
        }
    }
}
