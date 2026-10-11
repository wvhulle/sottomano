use std::{path::Path, time::Duration};

use tokio::task::spawn_blocking;
use uom::si::f64::ThermodynamicTemperature;

use super::{
    Error,
    btrfs::{BtrfsFilesystem, DeviceErrors},
    mount::{KernelDevice, RootMount},
};
use crate::{
    config::widget_groups::temperature_sensors::{SensorChoice, SensorConfig, ThresholdOverrides},
    data::source::{
        Source,
        hardware::DriveIdentity,
        sysfs_sensors::hwmon::{ChosenSensors, HwmonChip, LimitKind},
    },
};

const COMPOSITE_LABEL: &str = "Composite";
const DRIVETEMP_MODULE: &str = "/sys/module/drivetemp";

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
