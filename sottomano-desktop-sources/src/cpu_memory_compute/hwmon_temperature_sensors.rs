use std::{
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use sottomano_toml_config::widget_groups::temperature_sensors::{SensorChoice, SensorLabel};
use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius};

use crate::system_protocol_clients::sysfs_attribute_reads::{list_dir, read_i64, read_trimmed};

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
    #[must_use]
    pub fn all() -> Vec<Self> {
        Self::in_dirs(list_dir(HWMON))
    }

    #[must_use]
    pub fn of_device(device: &Path) -> Vec<Self> {
        let multipath_controllers = list_dir(device)
            .into_iter()
            .filter(|entry| numbered_suffix(entry, "nvme").is_some());
        let candidates = [device.to_path_buf()]
            .into_iter()
            .chain(multipath_controllers)
            .flat_map(|dir| {
                list_dir(&dir)
                    .into_iter()
                    .filter(|entry| numbered_suffix(entry, "hwmon").is_some())
                    .chain(list_dir(dir.join("hwmon")))
            })
            .collect();
        Self::in_dirs(candidates)
    }

    #[must_use]
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

    #[must_use]
    pub fn device_name(&self) -> Option<String> {
        Some(
            self.dir
                .join("device")
                .canonicalize()
                .inspect_err(|resolve_failure| {
                    tracing::debug!(
                        "hwmon: cannot resolve device of {}: {resolve_failure}",
                        self.dir.display()
                    );
                })
                .ok()?
                .file_name()?
                .to_str()?
                .to_owned(),
        )
    }

    #[must_use]
    pub fn temperature_sensors(&self) -> Vec<TemperatureSensor> {
        let mut sensors: Vec<TemperatureSensor> = list_dir(&self.dir)
            .iter()
            .filter_map(|path| {
                let index_digits = path
                    .file_name()?
                    .to_str()?
                    .strip_prefix("temp")?
                    .strip_suffix("_input")?;
                let index = index_digits
                    .parse()
                    .inspect_err(|parse_failure| {
                        tracing::warn!(
                            "hwmon: invalid temperature sensor index in {}: {parse_failure}",
                            path.display()
                        );
                    })
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

    #[must_use]
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

    pub fn limit(
        &self,
        sensor: &TemperatureSensor,
        kind: LimitKind,
    ) -> Option<ThermodynamicTemperature> {
        let suffix = match kind {
            LimitKind::Max => "max",
            LimitKind::Crit => "crit",
        };
        self.millidegrees(sensor, suffix)
            .map(celsius_from_millidegrees)
            .filter(|limit| PLAUSIBLE_LIMIT_CELSIUS.contains(&limit.get::<degree_celsius>()))
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

fn numbered_suffix(entry: &Path, prefix: &str) -> Option<u32> {
    entry
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(prefix))
        .filter(|digits| digits.bytes().all(|digit_byte| digit_byte.is_ascii_digit()))
        .and_then(|digits| {
            digits
                .parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "hwmon: invalid {prefix} number in {}: {parse_failure}",
                        entry.display()
                    );
                })
                .ok()
        })
}

#[must_use]
pub fn celsius_from_millidegrees(millidegrees: i64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(millidegrees as f64 / 1000.0)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, process};

    use super::*;

    fn multipath_nvme_subsystem() -> PathBuf {
        let subsystem = env::temp_dir().join(format!("sottomano-hwmon-{}", process::id()));
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
        assert_eq!(max, None);
        assert_eq!(crit, Some(celsius_from_millidegrees(84_850)));
    }
}
