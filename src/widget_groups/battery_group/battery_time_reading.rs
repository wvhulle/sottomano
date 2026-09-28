use iced::Task;

use crate::{
    data::{
        battery_forecast::{ChargeCurve, Estimate, bin_of, time_until_full},
        source::sysfs_sensors::battery::{Battery, IdleReason},
    },
    state::charge_power_curve,
    widget_groups::battery_group::estimate_columns::{
        ESTIMATE_HEADINGS, duration_text, estimate_cells,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::Solid,
        tooltip_body::{Cell, TableRow, TooltipBody, row},
    },
};

const MIN_SAMPLE_W: f64 = 0.5;

#[derive(Default)]
pub struct BatteryTimeReading {
    curve: Option<ChargeCurve>,
    load_started: bool,
}

#[derive(Debug, Clone)]
pub struct CurveLoaded(Box<ChargeCurve>);

impl BatteryTimeReading {
    pub fn learn(&mut self, battery: &Battery) -> Task<CurveLoaded> {
        let Some(curve) = self.curve.take() else {
            return self.start_loading(&battery.serial);
        };
        let sample = battery.charging_sample(MIN_SAMPLE_W);
        let curve = match sample {
            Some(sample) => curve.observe(sample.state_of_charge, sample.power_w),
            None => curve.interrupted(),
        };
        let save = sample.map_or_else(Task::none, |sample| {
            Task::future(charge_power_curve::save(
                battery.serial.clone(),
                bin_of(sample.state_of_charge),
                curve.clone(),
            ))
            .discard()
        });
        self.curve = Some(curve);
        save
    }

    pub fn loaded(&mut self, CurveLoaded(curve): CurveLoaded) {
        self.curve = Some(*curve);
    }

    fn start_loading(&mut self, serial: &str) -> Task<CurveLoaded> {
        if self.load_started || serial.is_empty() {
            return Task::none();
        }
        self.load_started = true;
        Task::future(charge_power_curve::load(serial.to_string())).map(CurveLoaded)
    }

    pub(super) fn time_for(
        &self,
        battery: &Battery,
        drain: Option<Estimate>,
    ) -> Option<BatteryTime> {
        let idle = battery.idle_reason().map(|reason| IdleBattery {
            reason,
            capacity: battery.capacity,
            limit_percent: battery.limit_percent(),
        });
        idle.map(BatteryTime::Idle)
            .or_else(|| self.countdown(battery, drain).map(BatteryTime::Countdown))
    }

    fn countdown(&self, battery: &Battery, drain: Option<Estimate>) -> Option<Countdown> {
        if battery.is_discharging() {
            return Some(Countdown::UntilEmpty(drain));
        }
        battery.is_charging().then(|| Countdown::UntilFull {
            limit_percent: battery.limit_percent(),
            estimate: self
                .curve
                .as_ref()
                .and_then(|curve| time_until_full(battery, curve)),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Countdown {
    UntilEmpty(Option<Estimate>),
    UntilFull {
        limit_percent: u32,
        estimate: Option<Estimate>,
    },
}

impl Countdown {
    fn estimate(self) -> Option<Estimate> {
        match self {
            Countdown::UntilEmpty(estimate) | Countdown::UntilFull { estimate, .. } => estimate,
        }
    }
}

impl TableRow for Countdown {
    const HEADINGS: &'static [&'static str] = ESTIMATE_HEADINGS;

    fn key(&self) -> Cell {
        match self {
            Countdown::UntilEmpty(_) => "Until empty".into(),
            Countdown::UntilFull { limit_percent, .. } => format!("Until {limit_percent} %").into(),
        }
    }

    fn cells(&self) -> Vec<Cell> {
        estimate_cells(self.estimate())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct IdleBattery {
    reason: IdleReason,
    capacity: u8,
    limit_percent: u32,
}

impl IdleBattery {
    fn icon(self) -> Glyph {
        match self.reason {
            IdleReason::Full | IdleReason::AtLimit => Solid::BatteryFull.into(),
            IdleReason::Inhibited => Solid::CirclePause.into(),
            IdleReason::Unexplained => Solid::PlugCircleExclamation.into(),
        }
    }

    fn label(self) -> &'static str {
        match self.reason {
            IdleReason::Full => "Full",
            IdleReason::AtLimit => "At limit",
            IdleReason::Inhibited => "Paused",
            IdleReason::Unexplained => "Not charging",
        }
    }

    fn explanation(self) -> &'static str {
        match self.reason {
            IdleReason::Full => "reported full by the battery",
            IdleReason::AtLimit => "held at the charge limit",
            IdleReason::Inhibited => "charging switched off (charge_behaviour)",
            IdleReason::Unexplained => "on AC, no cause reported by the battery",
        }
    }

    fn tooltip(self) -> TooltipBody {
        TooltipBody::rows(vec![
            row("Status", self.explanation()),
            row("Charge", format!("{} %", self.capacity)),
            row("Charge limit", format!("{} %", self.limit_percent)),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum BatteryTime {
    Idle(IdleBattery),
    Countdown(Countdown),
}

impl BarWidget for BatteryTime {
    fn icon(&self) -> Glyph {
        match self {
            BatteryTime::Idle(idle) => idle.icon(),
            BatteryTime::Countdown(_) => Solid::HourglassHalf.into(),
        }
    }

    fn label(&self) -> Option<String> {
        match self {
            BatteryTime::Idle(idle) => Some(idle.label().to_string()),
            BatteryTime::Countdown(countdown) => Some(duration_text(countdown.estimate()?.time)),
        }
    }

    fn tooltip(&self) -> TooltipBody {
        match self {
            BatteryTime::Idle(idle) => idle.tooltip(),
            BatteryTime::Countdown(countdown) => TooltipBody::table([*countdown]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_key_names_the_direction() {
        let until_full = Countdown::UntilFull {
            limit_percent: 80,
            estimate: None,
        };
        assert_eq!(Countdown::UntilEmpty(None).key(), "Until empty".into());
        assert_eq!(until_full.key(), "Until 80 %".into());
    }

    #[test]
    fn idle_battery_is_labelled_by_its_reason() {
        let idle = |reason| {
            BatteryTime::Idle(IdleBattery {
                reason,
                capacity: 99,
                limit_percent: 100,
            })
            .label()
        };
        assert_eq!(idle(IdleReason::Full).as_deref(), Some("Full"));
        assert_eq!(idle(IdleReason::AtLimit).as_deref(), Some("At limit"));
        assert_eq!(idle(IdleReason::Inhibited).as_deref(), Some("Paused"));
        assert_eq!(
            idle(IdleReason::Unexplained).as_deref(),
            Some("Not charging")
        );
    }
}
