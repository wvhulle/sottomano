use std::time::{Duration, Instant};

use chrono::Locale;
use iced::Task;
use sottomano_config::severity::Severity;
use sottomano_sources::battery_power_profiles::{
    battery_time_forecast::{ChargeCurve, Estimate, bin_of, time_until_full},
    upower_battery_state::{Battery, IdleReason},
};
use uom::si::{
    f64::{Power, Ratio},
    power::watt,
};

use crate::{
    clock::Clock,
    widget_groups::power_group::{
        charge_power_curve_store,
        estimate_columns::{ESTIMATE_HEADINGS, Forecast, Unforecastable},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        gauge::percent_text,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TableRecord, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

const MIN_SAMPLE_W: f64 = 0.5;
const EMPTY_CRITICAL: Duration = Duration::from_mins(10);
const EMPTY_WARNING: Duration = Duration::from_mins(20);

#[derive(Default)]
pub struct BatteryTimeReading {
    curve: Option<ChargeCurve>,
    load_started: bool,
    previous_sample_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct CurveLoaded(Box<ChargeCurve>);

impl BatteryTimeReading {
    pub fn learn(&mut self, battery: &Battery, now: Instant) -> Task<CurveLoaded> {
        let Some(curve) = &self.curve else {
            return self.start_loading(&battery.serial);
        };
        let Some(sample) = battery.charging_sample(Power::new::<watt>(MIN_SAMPLE_W)) else {
            self.previous_sample_at = None;
            return Task::none();
        };
        let since_previous = self
            .previous_sample_at
            .replace(now)
            .map_or(Duration::ZERO, |previous| now - previous);
        let curve = curve
            .clone()
            .observe(sample.state_of_charge, sample.power, since_previous);
        self.curve = Some(curve.clone());
        Task::future(charge_power_curve_store::save(
            battery.serial.clone(),
            bin_of(sample.state_of_charge),
            curve,
        ))
        .discard()
    }

    pub fn loaded(&mut self, CurveLoaded(curve): CurveLoaded) {
        self.curve = Some(*curve);
    }

    fn start_loading(&mut self, serial: &str) -> Task<CurveLoaded> {
        if self.load_started || serial.is_empty() {
            return Task::none();
        }
        self.load_started = true;
        Task::future(charge_power_curve_store::load(serial.to_string())).map(CurveLoaded)
    }

    pub(super) fn time_for(
        &self,
        battery: &Battery,
        drain: Option<Estimate>,
    ) -> Option<BatteryTime> {
        let idle = battery.idle_reason().map(|reason| IdleBattery {
            reason,
            capacity: battery.capacity,
            charge_limit: battery.charge_limit,
        });
        idle.map(BatteryTime::Idle)
            .or_else(|| self.countdown(battery, drain).map(BatteryTime::Countdown))
    }

    fn countdown(&self, battery: &Battery, drain: Option<Estimate>) -> Option<Countdown> {
        if battery.is_discharging() {
            return Some(Countdown::UntilEmpty(Forecast::from_estimate(
                drain,
                Unforecastable::NoDrainHistory,
            )));
        }
        battery.is_charging().then(|| Countdown::UntilFull {
            charge_limit: battery.charge_limit,
            forecast: self.charge_forecast(battery),
        })
    }

    fn charge_forecast(&self, battery: &Battery) -> Forecast {
        let Some(curve) = &self.curve else {
            return Forecast::Unavailable(Unforecastable::LoadingHistory);
        };
        if !curve.has_history() {
            return Forecast::Unavailable(Unforecastable::NoChargeHistory);
        }
        Forecast::from_estimate(
            time_until_full(battery, curve),
            Unforecastable::ChargerNotKeepingUp,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Countdown {
    UntilEmpty(Forecast),
    UntilFull {
        charge_limit: Ratio,
        forecast: Forecast,
    },
}

impl Countdown {
    fn forecast(self) -> Forecast {
        match self {
            Countdown::UntilEmpty(forecast) | Countdown::UntilFull { forecast, .. } => forecast,
        }
    }
}

impl Countdown {
    fn key(self) -> String {
        match self {
            Countdown::UntilEmpty(_) => "Until empty".to_owned(),
            Countdown::UntilFull { charge_limit, .. } => {
                format!("Until {}", percent_text(charge_limit))
            }
        }
    }
}

impl TableRow<3> for Countdown {
    fn table_record(self) -> TableRecord<3> {
        TableRecord::new(self.key(), self.forecast().cells())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct IdleBattery {
    reason: IdleReason,
    capacity: Ratio,
    charge_limit: Ratio,
}

impl IdleBattery {
    fn icon(self) -> Glyph {
        match self.reason {
            IdleReason::Full | IdleReason::AtLimit => Phosphor::BatteryFull(Weight::Regular).into(),
            IdleReason::Inhibited => Phosphor::PauseCircle(Weight::Regular).into(),
            IdleReason::Unexplained => Phosphor::PlugCharging(Weight::Regular).into(),
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
        TooltipBody::dictionary([
            KeyValue::new("Status", self.explanation()),
            KeyValue::new("Charge", percent_text(self.capacity)),
            KeyValue::new("Charge limit", percent_text(self.charge_limit)),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum BatteryTime {
    Idle(IdleBattery),
    Countdown(Countdown),
}

impl BarWidget for BatteryTime {
    fn name(&self) -> &'static str {
        "Battery time"
    }

    fn icon(&self) -> Glyph {
        match self {
            BatteryTime::Idle(idle) => idle.icon(),
            BatteryTime::Countdown(_) => Phosphor::HourglassMedium(Weight::Regular).into(),
        }
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        match self {
            BatteryTime::Idle(idle) => Some(idle.label().to_string()),
            BatteryTime::Countdown(countdown) => Some(countdown.forecast().label(locale)),
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        match self {
            BatteryTime::Countdown(Countdown::UntilEmpty(Forecast::Estimated(estimate)))
                if estimate.time < EMPTY_CRITICAL =>
            {
                WidgetState::Critical
            }
            BatteryTime::Countdown(Countdown::UntilEmpty(Forecast::Estimated(estimate)))
                if estimate.time < EMPTY_WARNING =>
            {
                WidgetState::Warning
            }
            BatteryTime::Idle(_) | BatteryTime::Countdown(_) => WidgetState::Idle,
        }
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "Battery running out",
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        match self {
            BatteryTime::Idle(idle) => idle.tooltip(),
            BatteryTime::Countdown(countdown) => {
                TooltipBody::sections([Section::column_table(ESTIMATE_HEADINGS, [*countdown])])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use uom::si::ratio::percent;

    use super::*;

    #[test]
    fn row_key_names_the_direction() {
        let until_full = Countdown::UntilFull {
            charge_limit: Ratio::new::<percent>(80.0),
            forecast: Forecast::Unavailable(Unforecastable::NoChargeHistory),
        };
        assert_eq!(
            Countdown::UntilEmpty(Forecast::Unavailable(Unforecastable::NoDrainHistory)).key(),
            "Until empty"
        );
        assert_eq!(until_full.key(), "Until 80 %");
    }

    #[test]
    fn idle_battery_is_labelled_by_its_reason() {
        let idle = |reason| {
            BatteryTime::Idle(IdleBattery {
                reason,
                capacity: Ratio::new::<percent>(99.0),
                charge_limit: Ratio::new::<percent>(100.0),
            })
            .label(Clock::real(), Locale::en_US)
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
