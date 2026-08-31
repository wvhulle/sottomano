use std::{
    future::{Future, ready},
    time::Duration,
};

use tokio::task::spawn_blocking;

use super::{
    Error, POWER_SUPPLY, battery::ChargeStatus, first_power_supply, parse_field, read_trimmed,
};
use crate::data::source::{
    BarWidget, Glyph, Level, Source, TooltipBody, Unit, Value, font_awesome::Solid, row,
};

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;
const POWER_GAUGE_MAX_W: f64 = 40.0;

#[derive(Debug, Clone, PartialEq)]
pub struct PowerDraw {
    pub watts: f64,
    pub status: ChargeStatus,
}

impl BarWidget for PowerDraw {
    fn icon(&self) -> Glyph {
        match self.status {
            ChargeStatus::Discharging | ChargeStatus::Unknown => Solid::PlugCircleMinus.into(),
            _ => Solid::PlugCirclePlus.into(),
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
        let pct = (self.watts / POWER_GAUGE_MAX_W * 100.0).clamp(0.0, 100.0);
        Some(Value::Gauge {
            percent: pct.round() as i64,
            unit: Unit::Watt,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let w = format!("{:.1} W", self.watts);
        TooltipBody::rows(match self.status {
            ChargeStatus::Charging => vec![row("Rate", w)],
            ChargeStatus::Full | ChargeStatus::NotCharging => vec![row("Power", "AC")],
            _ => vec![row("Draw", w)],
        })
    }
}

impl Source for PowerDraw {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "power-consumption";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn available(_key: &()) -> impl Future<Output = bool> + Send {
        ready(first_power_supply("BAT").is_some())
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        spawn_blocking(read_power_draw)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_power_draw() -> Result<PowerDraw, Error> {
    let dir =
        first_power_supply("BAT").ok_or_else(|| Error::Unreadable(POWER_SUPPLY.to_owned()))?;
    let uw: f64 = parse_field(&dir.join("power_now"))?;
    let status =
        read_trimmed(dir.join("status")).map_or(ChargeStatus::Unknown, |s| ChargeStatus::parse(&s));
    Ok(PowerDraw {
        watts: uw / 1_000_000.0,
        status,
    })
}
