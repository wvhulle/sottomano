use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TemperatureSensorsConfig {
    pub cpu: SensorConfig,
    pub drive: SensorConfig,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SensorConfig {
    pub sensor: SensorChoice,
    pub warning_celsius: Threshold,
    pub critical_celsius: Threshold,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ThresholdOverrides {
    pub warning: Threshold,
    pub critical: Threshold,
}

impl SensorConfig {
    #[must_use]
    pub fn overrides(&self) -> ThresholdOverrides {
        ThresholdOverrides {
            warning: self.warning_celsius,
            critical: self.critical_celsius,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(from = "ChipSpec")]
pub enum SensorChoice {
    #[default]
    Automatic,
    Chip {
        chip: String,
        label: SensorLabel,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SensorLabel {
    Any,
    Named(String),
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, Hash)]
#[serde(from = "u16")]
pub enum Threshold {
    #[default]
    FromHardware,
    Celsius(u16),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChipSpec {
    chip: String,
    label: Option<String>,
}

impl From<ChipSpec> for SensorChoice {
    fn from(spec: ChipSpec) -> Self {
        SensorChoice::Chip {
            chip: spec.chip,
            label: spec.label.map_or(SensorLabel::Any, SensorLabel::Named),
        }
    }
}

impl From<u16> for Threshold {
    fn from(celsius: u16) -> Self {
        Threshold::Celsius(celsius)
    }
}

impl Serialize for Threshold {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Threshold::FromHardware => serializer.serialize_none(),
            Threshold::Celsius(celsius) => serializer.serialize_some(celsius),
        }
    }
}

impl Serialize for SensorChoice {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            SensorChoice::Automatic => serializer.serialize_none(),
            SensorChoice::Chip { chip, label } => serializer.serialize_some(&ChipSpec {
                chip: chip.clone(),
                label: match label {
                    SensorLabel::Any => None,
                    SensorLabel::Named(name) => Some(name.clone()),
                },
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml: &str) -> TemperatureSensorsConfig {
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn omitted_settings_detect_sensors_and_use_hardware_limits() {
        let config = parse("");
        assert_eq!(config.cpu.sensor, SensorChoice::Automatic);
        assert_eq!(config.drive.critical_celsius, Threshold::FromHardware);
    }

    #[test]
    fn a_chip_can_be_pinned_with_or_without_a_label() {
        let config = parse(
            "[cpu]\nsensor = { chip = \"k10temp\", label = \"Tctl\" }\n[drive]\nsensor = { chip = \
             \"drivetemp\" }\ncritical_celsius = 60\n",
        );
        assert_eq!(
            config.cpu.sensor,
            SensorChoice::Chip {
                chip: "k10temp".into(),
                label: SensorLabel::Named("Tctl".into()),
            }
        );
        assert_eq!(
            config.drive.sensor,
            SensorChoice::Chip {
                chip: "drivetemp".into(),
                label: SensorLabel::Any,
            }
        );
        assert_eq!(config.drive.critical_celsius, Threshold::Celsius(60));
        assert_eq!(config.drive.warning_celsius, Threshold::FromHardware);
    }
}
