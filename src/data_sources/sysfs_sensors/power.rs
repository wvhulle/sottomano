//! Live system power draw, ported from odin's `power-consumption.nu`.

use std::path::Path;

use crate::data_sources::{BarWidget, Level, Row, Source, Unit, Value, row};

use super::battery::ChargeStatus;
use super::read_trimmed;

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;
const POWER_GAUGE_MAX_W: f64 = 40.0;

/// Live system power draw in watts (`power-consumption.nu`), from `BAT0/power_now`.
#[derive(Debug, Clone)]
pub struct PowerDraw {
    pub watts: f64,
    pub status: ChargeStatus,
}

impl BarWidget for PowerDraw {
    fn icon(&self) -> &'static str {
        match self.status {
            ChargeStatus::Discharging | ChargeStatus::Unknown => "plug-circle-minus",
            _ => "plug-circle-plus",
        }
    }

    fn level(&self) -> Level {
        if self.status != ChargeStatus::Discharging {
            Level::Ok
        } else if self.watts > POWER_CRITICAL_W {
            Level::Critical
        } else if self.watts > POWER_WARN_W {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        // Map 0..GAUGE_MAX watts onto 0..100.
        let pct = (self.watts / POWER_GAUGE_MAX_W * 100.0).clamp(0.0, 100.0);
        Some(Value::Gauge {
            percent: pct.round() as i64,
            unit: Unit::Watt,
        })
    }

    fn tooltip(&self) -> Vec<Row> {
        let w = format!("{:.1} W", self.watts);
        match self.status {
            ChargeStatus::Charging => vec![row("Rate", w)],
            ChargeStatus::Full | ChargeStatus::NotCharging => vec![row("Power", "AC")],
            _ => vec![row("Draw", w)],
        }
    }
}

impl Source for PowerDraw {
    const NAME: &'static str = "power-consumption";

    fn poll() -> Option<Self> {
        let dir = Path::new("/sys/class/power_supply/BAT0");
        let uw: f64 = read_trimmed(dir.join("power_now"))?.parse().ok()?;
        let status = read_trimmed(dir.join("status"))
            .map(|s| ChargeStatus::parse(&s))
            .unwrap_or(ChargeStatus::Unknown);
        Some(PowerDraw {
            watts: uw / 1_000_000.0,
            status,
        })
    }
}
