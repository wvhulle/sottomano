use crate::{
    data::source::sysfs_sensors::battery::{Battery, ChargeStatus},
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

const POWER_WARN_W: f64 = 10.0;
const POWER_CRITICAL_W: f64 = 20.0;
const POWER_GAUGE_MAX_W: f64 = 40.0;

pub struct PowerDraw {
    watts: f64,
    status: ChargeStatus,
}

impl PowerDraw {
    pub fn of(battery: &Battery) -> Option<Self> {
        Some(PowerDraw {
            watts: battery.power_w?,
            status: battery.status,
        })
    }
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
        let fraction = (self.watts / POWER_GAUGE_MAX_W * 100.0).clamp(0.0, 100.0);
        Some(Value::Gauge {
            percent: fraction.round() as i64,
            unit: Unit::Watt,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let watts = format!("{:.1} W", self.watts);
        TooltipBody::rows(match self.status {
            ChargeStatus::Charging => vec![row("Rate", watts)],
            ChargeStatus::Full | ChargeStatus::NotCharging => vec![row("Power", "AC")],
            _ => vec![row("Draw", watts)],
        })
    }
}
