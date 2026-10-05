use std::{
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius};

use super::{list_dir, read_i64, read_trimmed, temperature::ThermalLimit};
use crate::config::temperature::{SensorChoice, SensorLabel};

const HWMON: &str = "/sys/class/hwmon";
const PLAUSIBLE_LIMIT_CELSIUS: RangeInclusive<f64> = 1.0..=150.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwmonChip {
    pub name: String,
    dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemperatureSensor {
    pub index: u32,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    Max,
    Crit,
}

impl HwmonChip {
    pub fn all() -> Vec<Self> {
        Self::in_dirs(list_dir(HWMON))
    }

    pub fn of_device(device: &Path) -> Vec<Self> {
        let multipath_controllers = list_dir(device)
            .into_iter()
            .filter(|entry| has_numbered_name(entry, "nvme"));
        let candidates = [device.to_path_buf()]
            .into_iter()
            .chain(multipath_controllers)
            .flat_map(|dir| {
                list_dir(&dir)
                    .into_iter()
                    .filter(|entry| has_numbered_name(entry, "hwmon"))
                    .chain(list_dir(dir.join("hwmon")))
            })
            .collect();
        Self::in_dirs(candidates)
    }

    pub fn chosen(choice: &SensorChoice) -> Vec<ChosenSensors> {
        match choice {
            SensorChoice::Automatic => Vec::new(),
            SensorChoice::Chip { chip, label } => Self::all()
                .into_iter()
                .filter(|candidate| &candidate.name == chip)
                .map(|candidate| {
                    let sensors = candidate.labelled(label);
                    ChosenSensors {
                        chip: candidate,
                        sensors,
                    }
                })
                .filter(|chosen| !chosen.sensors.is_empty())
                .collect(),
        }
    }

    fn in_dirs(mut dirs: Vec<PathBuf>) -> Vec<Self> {
        dirs.sort();
        dirs.dedup();
        dirs.into_iter()
            .filter_map(|dir| {
                Some(HwmonChip {
                    name: read_trimmed(dir.join("name"))?,
                    dir,
                })
            })
            .collect()
    }

    pub fn temperature_sensors(&self) -> Vec<TemperatureSensor> {
        let mut sensors: Vec<TemperatureSensor> = list_dir(&self.dir)
            .iter()
            .filter_map(|path| {
                let index = path
                    .file_name()?
                    .to_str()?
                    .strip_prefix("temp")?
                    .strip_suffix("_input")?
                    .parse()
                    .ok()?;
                Some(TemperatureSensor {
                    index,
                    label: read_trimmed(self.dir.join(format!("temp{index}_label")))
                        .unwrap_or_else(|| format!("temp{index}")),
                })
            })
            .collect();
        sensors.sort_by_key(|sensor| sensor.index);
        sensors
    }

    pub fn labelled(&self, label: &SensorLabel) -> Vec<TemperatureSensor> {
        self.temperature_sensors()
            .into_iter()
            .filter(|sensor| match label {
                SensorLabel::Any => true,
                SensorLabel::Named(name) => &sensor.label == name,
            })
            .collect()
    }

    pub fn temperature(&self, sensor: &TemperatureSensor) -> Option<ThermodynamicTemperature> {
        self.millidegrees(sensor, "input")
            .map(celsius_from_millidegrees)
    }

    pub fn limit(&self, sensor: &TemperatureSensor, kind: LimitKind) -> ThermalLimit {
        let suffix = match kind {
            LimitKind::Max => "max",
            LimitKind::Crit => "crit",
        };
        self.millidegrees(sensor, suffix)
            .map(celsius_from_millidegrees)
            .filter(|limit| PLAUSIBLE_LIMIT_CELSIUS.contains(&limit.get::<degree_celsius>()))
            .map_or(ThermalLimit::Unreported, ThermalLimit::Reported)
    }

    fn millidegrees(&self, sensor: &TemperatureSensor, suffix: &str) -> Option<i64> {
        read_i64(self.dir.join(format!("temp{}_{suffix}", sensor.index)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChosenSensors {
    pub chip: HwmonChip,
    pub sensors: Vec<TemperatureSensor>,
}

fn has_numbered_name(entry: &Path, prefix: &str) -> bool {
    entry
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(prefix))
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

pub fn celsius_from_millidegrees(millidegrees: i64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(millidegrees as f64 / 1000.0)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use super::*;

    fn multipath_nvme_subsystem() -> PathBuf {
        let subsystem = env::temp_dir().join(format!("vetrata-hwmon-{}", process::id()));
        let chip = subsystem.join("nvme0").join("hwmon3");
        fs::create_dir_all(&chip).unwrap();
        for (file, contents) in [
            ("name", "nvme"),
            ("temp1_input", "35850"),
            ("temp1_label", "Composite"),
            ("temp1_max", "65261850"),
            ("temp1_crit", "84850"),
            ("temp2_input", "41850"),
        ] {
            fs::write(chip.join(file), contents).unwrap();
        }
        subsystem
    }

    #[test]
    fn chips_behind_multipath_controllers_are_found_with_plausible_limits() {
        let subsystem = multipath_nvme_subsystem();
        let chips = HwmonChip::of_device(&subsystem);
        let sensors = chips.first().map(HwmonChip::temperature_sensors);
        let composite = chips.first().zip(sensors.as_ref()).map(|(chip, sensors)| {
            let composite = &sensors[0];
            (
                chip.limit(composite, LimitKind::Max),
                chip.limit(composite, LimitKind::Crit),
            )
        });
        fs::remove_dir_all(&subsystem).unwrap();

        let sensors = sensors.unwrap();
        assert_eq!(sensors[0].label, "Composite");
        assert_eq!(sensors[1].label, "temp2");
        let (max, crit) = composite.unwrap();
        assert_eq!(max, ThermalLimit::Unreported);
        assert_eq!(
            crit,
            ThermalLimit::Reported(celsius_from_millidegrees(84_850))
        );
    }
}
