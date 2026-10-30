use std::{path::Path, time::Duration};

use futures::Stream;
use sottomano_config::{
    severity::Severity,
    widget_groups::temperature_sensors::{SensorChoice, SensorConfig, ThresholdOverrides},
};
use uom::si::f64::ThermodynamicTemperature;

use super::{
    Error,
    bcachefs_filesystem_space::BcachefsFilesystem,
    block_filesystem_mounts::{BlockFilesystemDiscovery, FilesystemMount, KernelDevice},
    btrfs_filesystem_space::BtrfsFilesystem,
    filesystem_device_errors::DeviceErrors,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking_poll,
    cpu_memory_compute::{
        hwmon_temperature_sensors::{ChosenSensors, HwmonChip, LimitKind},
        temperature_thresholds::{TemperatureThresholds, ThresholdBasis, celsius},
    },
    delivery_cadence_drivers::PeriodicPolling,
    host_machine_identity::udev_hardware_identity::DriveIdentity,
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
    pub errors: Vec<DeviceErrors>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DriveHealthInputs {
    pub disk: KernelDevice,
    pub sensor: SensorConfig,
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

impl SnapshotSource for DriveHealth {
    type Error = Error;
    type StreamInputs = DriveHealthInputs;

    const NAME: &'static str = "Drive health";

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(Duration::from_secs(60))
            .poll_deliveries(move || Self::poll(inputs.clone()))
    }
}

impl DriveHealth {
    async fn poll(inputs: DriveHealthInputs) -> Result<Self, Error> {
        blocking_poll(move || read_drive_health(&inputs)).await
    }
}

fn read_drive_health(inputs: &DriveHealthInputs) -> Result<DriveHealth, Error> {
    let disk = &inputs.disk;
    let controller = disk.sysfs_dir().join("device");
    let discovery = BlockFilesystemDiscovery::read_mountinfo()?;
    let sensor_choice = if discovery.root_disks().contains(disk) {
        inputs.sensor.sensor.clone()
    } else {
        SensorChoice::Automatic
    };
    let errors = discovery
        .filesystems
        .iter()
        .filter(|filesystem| filesystem.sits_on_disk(disk))
        .map(|filesystem| errors_on_disk(filesystem, disk))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();
    Ok(DriveHealth {
        device_name: disk.name.clone(),
        identity: DriveIdentity::of_controller(&controller),
        temperature: DriveTemperature::read(&sensor_choice, disk, &controller),
        overrides: inputs.sensor.overrides(),
        errors,
    })
}

fn errors_on_disk(
    filesystem: &FilesystemMount,
    disk: &KernelDevice,
) -> Result<Vec<DeviceErrors>, Error> {
    match filesystem.fs_type.as_str() {
        "btrfs" => Ok(vec![
            BtrfsFilesystem::containing(&filesystem.devices)?.device_errors()?,
        ]),
        "bcachefs" => {
            let bcachefs = BcachefsFilesystem::containing(&filesystem.devices)?;
            filesystem
                .devices
                .iter()
                .filter(|partition| &partition.whole_disk() == disk)
                .map(|partition| bcachefs.device_errors(partition))
                .collect()
        }
        _ => Ok(Vec::new()),
    }
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
