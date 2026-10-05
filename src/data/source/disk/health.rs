use std::{path::Path, time::Duration};

use tokio::task::spawn_blocking;
use uom::si::f64::ThermodynamicTemperature;

use super::{
    Error,
    btrfs::{BtrfsFilesystem, DeviceErrors},
    mount::{KernelDevice, RootMount},
};
use crate::{
    config::temperature::{SensorChoice, SensorConfig, ThresholdOverrides},
    data::source::{
        Source,
        sysfs_sensors::{
            hwmon::{ChosenSensors, HwmonChip, LimitKind},
            read_trimmed,
            temperature::ThermalLimit,
        },
    },
};

const COMPOSITE_LABEL: &str = "Composite";
const DRIVETEMP_MODULE: &str = "/sys/module/drivetemp";

#[derive(Debug, Clone, PartialEq)]
pub struct DriveHealth {
    pub model: String,
    pub temperature: DriveTemperature,
    pub overrides: ThresholdOverrides,
    pub errors: ErrorTracking,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveTemperature {
    Reported {
        current: ThermodynamicTemperature,
        warning: ThermalLimit,
        critical: ThermalLimit,
    },
    Unreported(MissingSensor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingSensor {
    DrivetempNotLoaded,
    NotExposed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorTracking {
    Tracked(DeviceErrors),
    NotTracked,
}

impl Source for DriveHealth {
    type Error = Error;

    type Key = SensorConfig;

    const NAME: &'static str = "drive_health";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll(config: SensorConfig) -> Result<Self, Self::Error> {
        spawn_blocking(move || read_drive_health(&config))
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_drive_health(config: &SensorConfig) -> Result<DriveHealth, Error> {
    let mount = RootMount::current()?;
    let device = mount.device()?;
    let disk = device.whole_disk();
    let controller = disk.sysfs_dir().join("device");
    Ok(DriveHealth {
        model: read_trimmed(controller.join("model")).unwrap_or_else(|| disk.name.clone()),
        temperature: DriveTemperature::read(&config.sensor, &disk, &controller),
        overrides: config.overrides(),
        errors: ErrorTracking::of(&mount, &device)?,
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

impl ErrorTracking {
    fn of(mount: &RootMount, device: &KernelDevice) -> Result<Self, Error> {
        if mount.fs_type != "btrfs" {
            return Ok(ErrorTracking::NotTracked);
        }
        let errors = BtrfsFilesystem::containing(device)?.device_errors()?;
        Ok(ErrorTracking::Tracked(errors))
    }
}
