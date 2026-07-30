//! Battery charge level and power-source state.

use std::time::Duration;

use super::{energy_wh, first_power_supply, read_trimmed};
use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

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
}

#[derive(Debug, Clone)]
pub struct Battery {
    pub capacity: u8,
    pub status: ChargeStatus,
    pub energy_wh: Option<f64>,
}

impl BarWidget for Battery {
    fn icon(&self) -> Glyph {
        Solid::CarBattery.into()
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
        let mut rows = vec![
            row("Battery", format!("{}%", self.capacity)),
            row("Status", self.status.as_str()),
        ];
        if let Some(wh) = self.energy_wh {
            rows.push(row("Remaining", format!("{wh:.1} Wh")));
        }
        TooltipBody::Table(rows)
    }
}

impl Source for Battery {
    const NAME: &'static str = "battery";
    const INTERVAL: Duration = Duration::from_secs(30);

    fn poll() -> Option<Self> {
        let dir = first_power_supply("BAT")?;
        let capacity = read_trimmed(dir.join("capacity"))?.parse().ok()?;
        let status = ChargeStatus::parse(&read_trimmed(dir.join("status"))?);
        Some(Battery {
            capacity,
            status,
            energy_wh: energy_wh(&dir),
        })
    }
}
