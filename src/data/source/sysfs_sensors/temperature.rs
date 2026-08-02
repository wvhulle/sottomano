use std::{path::Path, time::Duration};

use super::{favg, fmax, fmin, list_dir, read_i64, read_trimmed};
use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

const CPU_CHIP_PREFIXES: [&str; 3] = ["coretemp", "k10temp", "zenpower"];
const TEMP_FALLBACK_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

#[derive(Debug, Clone, PartialEq)]
pub struct TempReading {
    pub chip: String,
    pub celsius: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    pub readings: Vec<TempReading>,
}

impl Temperature {
    fn temps(&self) -> Vec<f64> {
        self.readings.iter().map(|r| r.celsius).collect()
    }
}

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        Solid::TemperatureHalf.into()
    }

    fn level(&self) -> Level {
        let avg = favg(&self.temps());
        if avg > 85.0 {
            Level::Critical
        } else if avg > 70.0 {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        let avg = favg(&self.temps()).round() as i64;
        Some(Value::Gauge {
            percent: avg.min(100),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let temps = self.temps();
        let mut chips: Vec<&str> = self.readings.iter().map(|r| r.chip.as_str()).collect();
        chips.dedup();
        TooltipBody::Table(vec![
            row("Chips", chips.join(", ")),
            row("Samples", self.readings.len().to_string()),
            row("Min", format!("{}°C", fmin(&temps).round() as i64)),
            row("Avg", format!("{}°C", favg(&temps).round() as i64)),
            row("Max", format!("{}°C", fmax(&temps).round() as i64)),
        ])
    }
}

impl Source for Temperature {
    const NAME: &'static str = "temperature";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn poll() -> Option<Self> {
        let readings = cpu_temp_readings();
        (!readings.is_empty()).then_some(Temperature { readings })
    }
}

fn cpu_temp_readings() -> Vec<TempReading> {
    let mut chips = list_dir("/sys/class/hwmon");
    chips.sort();
    chips
        .iter()
        .filter_map(|chip| {
            let name = read_trimmed(chip.join("name"))?;
            let is_coretemp = name.starts_with("coretemp");
            CPU_CHIP_PREFIXES
                .iter()
                .any(|p| name.starts_with(p))
                .then(|| chip_readings(chip, &name, is_coretemp))
        })
        .flatten()
        .collect()
}

fn chip_readings(chip: &Path, name: &str, is_coretemp: bool) -> Vec<TempReading> {
    let labels: Vec<(u32, String)> = temp_labels(chip);
    let is_core = |label: &str| {
        if is_coretemp {
            label.starts_with("Core ")
        } else {
            label.starts_with("Tccd")
        }
    };

    let indices: Vec<u32> = if labels.iter().any(|(_, l)| is_core(l)) {
        labels
            .iter()
            .filter(|(_, l)| is_core(l))
            .map(|(i, _)| *i)
            .collect()
    } else {
        TEMP_FALLBACK_LABELS
            .iter()
            .find_map(|fb| labels.iter().find(|(_, l)| l == fb).map(|(i, _)| *i))
            .into_iter()
            .collect()
    };

    indices
        .iter()
        .filter_map(|i| read_i64(chip.join(format!("temp{i}_input"))))
        .map(|milli| TempReading {
            chip: name.to_string(),
            celsius: milli as f64 / 1000.0,
        })
        .collect()
}

fn temp_labels(chip: &Path) -> Vec<(u32, String)> {
    list_dir(chip)
        .iter()
        .filter_map(|p| {
            let stem = p.file_name()?.to_str()?;
            let index: u32 = stem
                .strip_prefix("temp")?
                .strip_suffix("_label")?
                .parse()
                .ok()?;
            Some((index, read_trimmed(p)?))
        })
        .collect()
}
