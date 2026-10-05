use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::task::spawn_blocking;
use uom::si::f64::ThermodynamicTemperature;

use super::{
    Error,
    btrfs::{BtrfsFilesystem, DeviceErrors},
    mount::{KernelDevice, RootMount},
};
use crate::data::source::{
    Source,
    sysfs_sensors::{
        list_dir, read_i64, read_trimmed,
        temperature::{ThermalLimit, celsius_from_millidegrees},
    },
};

const COMPOSITE_SENSOR: &str = "temp1";

#[derive(Debug, Clone, PartialEq)]
pub struct DriveHealth {
    pub model: String,
    pub temperature: DriveTemperature,
    pub errors: ErrorTracking,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriveTemperature {
    Reported {
        current: ThermodynamicTemperature,
        warning: ThermalLimit,
        critical: ThermalLimit,
    },
    Unreported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorTracking {
    Tracked(DeviceErrors),
    NotTracked,
}

impl Source for DriveHealth {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "drive_health";
    const INTERVAL: Duration = Duration::from_secs(60);

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_drive_health)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_drive_health() -> Result<DriveHealth, Error> {
    let mount = RootMount::current()?;
    let device = mount.device()?;
    let disk = device.whole_disk();
    let controller = disk.sysfs_dir().join("device");
    Ok(DriveHealth {
        model: read_trimmed(controller.join("model")).unwrap_or_else(|| disk.name.clone()),
        temperature: DriveTemperature::of_controller(&controller),
        errors: ErrorTracking::of(&mount, &device)?,
    })
}

impl DriveTemperature {
    fn of_controller(controller: &Path) -> Self {
        sensor_chips(controller)
            .into_iter()
            .find_map(|chip| Self::of_chip(&chip))
            .unwrap_or(DriveTemperature::Unreported)
    }

    fn of_chip(chip: &Path) -> Option<Self> {
        let sensor = |suffix: &str| {
            read_i64(chip.join(format!("{COMPOSITE_SENSOR}_{suffix}")))
                .map(celsius_from_millidegrees)
        };
        let limit =
            |suffix: &str| sensor(suffix).map_or(ThermalLimit::Unreported, ThermalLimit::Reported);
        Some(DriveTemperature::Reported {
            current: sensor("input")?,
            warning: limit("max"),
            critical: limit("crit"),
        })
    }
}

fn sensor_chips(controller: &Path) -> Vec<PathBuf> {
    let mut chips: Vec<PathBuf> = list_dir(controller)
        .into_iter()
        .filter(|entry| {
            entry
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("hwmon") && name != "hwmon")
        })
        .chain(list_dir(controller.join("hwmon")))
        .collect();
    chips.sort();
    chips
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
