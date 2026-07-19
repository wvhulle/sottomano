//! Battery charge level + power-source state, ported from odin's `battery.nu`.

use super::{first_power_supply, read_trimmed};
use crate::data_sources::{BarWidget, Level, Row, Source, Unit, Value, row};

/// Power-source charge state, from `/sys/class/power_supply/*/status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl ChargeStatus {
    pub(super) fn parse(s: &str) -> Self {
        match s {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Full => "Full",
            Self::NotCharging => "Not charging",
            Self::Unknown => "Unknown",
        }
    }

    /// odin treats Full as "on AC / charged" — same logo as charging.
    fn charging(self) -> bool {
        matches!(self, Self::Charging | Self::Full)
    }
}

/// Battery charge level + power-source state (`battery.nu`).
#[derive(Debug, Clone)]
pub struct Battery {
    pub capacity: u8,
    pub status: ChargeStatus,
}

impl BarWidget for Battery {
    /// Two-state logo for the power source; the level is shown by the gauge.
    fn icon(&self) -> &'static str {
        if self.status.charging() {
            "plug-circle-bolt"
        } else {
            "car-battery"
        }
    }

    fn level(&self) -> Level {
        match self.capacity {
            0..=15 => Level::Critical,
            16..=30 => Level::Warning,
            _ => Level::Ok,
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.capacity as i64,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> Vec<Row> {
        vec![
            row("Battery", format!("{}%", self.capacity)),
            row("Status", self.status.as_str()),
        ]
    }
}

impl Source for Battery {
    const NAME: &'static str = "battery";

    fn poll() -> Option<Self> {
        let dir = first_power_supply("BAT")?;
        let capacity = read_trimmed(dir.join("capacity"))?.parse().ok()?;
        let status = ChargeStatus::parse(&read_trimmed(dir.join("status"))?);
        Some(Battery { capacity, status })
    }
}
