//! Battery charge level and power-source state.

use std::time::Duration;

use super::{first_power_supply, read_trimmed};
use crate::data_sources::{BarWidget, Icon, Level, Source, TooltipBody, Unit, Value, row};

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

    fn charging(self) -> bool {
        matches!(self, Self::Charging | Self::Full)
    }
}

#[derive(Debug, Clone)]
pub struct Battery {
    pub capacity: u8,
    pub status: ChargeStatus,
}

impl BarWidget for Battery {
    fn icon(&self) -> Icon {
        if self.status.charging() {
            Icon::PlugCircleBolt
        } else {
            Icon::CarBattery
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
            percent: i64::from(self.capacity),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![
            row("Battery", format!("{}%", self.capacity)),
            row("Status", self.status.as_str()),
        ])
    }
}

impl Source for Battery {
    const NAME: &'static str = "battery";
    const INTERVAL: Duration = Duration::from_secs(30);

    fn poll() -> Option<Self> {
        let dir = first_power_supply("BAT")?;
        let capacity = read_trimmed(dir.join("capacity"))?.parse().ok()?;
        let status = ChargeStatus::parse(&read_trimmed(dir.join("status"))?);
        Some(Battery { capacity, status })
    }
}
