use std::time::{Duration, Instant};

use chrono::Locale;
use iced::Task;
use sottomano_desktop_sources::battery_power_profiles::{
    battery_time_forecast::{ChargeCurve, CurveHistory, Estimate, bin_of, time_until_full},
    upower_battery_state::{Battery, ChargeStatus, IdleReason},
};
use sottomano_toml_config::severity::Severity;
use uom::si::{
    f64::{Power, Ratio},
    power::watt,
};

use crate::{
    group_indicators::{
        gauge::percent_text,
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table, headed_table},
    indicator_groups::power_group::{
        estimate_columns::{ESTIMATE_HEADINGS, Forecast, Unforecastable},
        sqlite_charge_power_curve,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::TableRecord,
        localized_quantities::Quantity,
        ongoing_toast::{IndicatorAlerts, StateToast, ToastBody, ToastProgress},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const MIN_SAMPLE_W: f64 = 0.5;
const EMPTY_CRITICAL: Duration = Duration::from_mins(10);
const EMPTY_WARNING: Duration = Duration::from_mins(20);

#[derive(Default)]
pub struct BatteryTimeIndicator {
    curve: Option<ChargeCurve>,
    load_started: bool,
    previous_sample_at: Option<Instant>,
}

impl BatteryTimeIndicator {
    pub fn learn(&mut self, battery: &Battery, now: Instant) -> Task<Box<ChargeCurve>> {
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
        Task::future(sqlite_charge_power_curve::save(
            battery.serial.clone(),
            bin_of(sample.state_of_charge),
            curve,
        ))
        .discard()
    }

    pub fn loaded(&mut self, curve: Box<ChargeCurve>) {
        self.curve = Some(*curve);
    }

    fn start_loading(&mut self, serial: &str) -> Task<Box<ChargeCurve>> {
        if self.load_started || serial.is_empty() {
            return Task::none();
        }
        self.load_started = true;
        Task::future(sqlite_charge_power_curve::load(serial.to_string()))
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
        match battery.status {
            ChargeStatus::Discharging => Some(Countdown::UntilEmpty(Forecast::estimated_or(
                drain,
                Unforecastable::NoDrainHistory,
            ))),
            ChargeStatus::Charging => Some(Countdown::UntilFull {
                charge_limit: battery.charge_limit,
                forecast: self.charge_forecast(battery),
            }),
            ChargeStatus::Full | ChargeStatus::NotCharging | ChargeStatus::Unreported => None,
        }
    }

    fn charge_forecast(&self, battery: &Battery) -> Forecast {
        let Some(curve) = &self.curve else {
            return Forecast::Unavailable(Unforecastable::LoadingHistory);
        };
        if curve.history() == CurveHistory::Empty {
            return Forecast::Unavailable(Unforecastable::NoChargeHistory);
        }
        Forecast::estimated_or(
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

impl From<Countdown> for TableRecord<3> {
    fn from(countdown: Countdown) -> Self {
        TableRecord::with_cells(countdown.key(), countdown.forecast().cells())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct IdleBattery {
    reason: IdleReason,
    capacity: Ratio,
    charge_limit: Ratio,
}

impl IdleBattery {
    fn icon(self) -> IconGlyph {
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

    fn hover_card(self) -> Vec<Section> {
        vec![Section::new(
            "Charge",
            dictionary_table([
                LabeledValue::new("Status", self.explanation()),
                LabeledValue::new("Charge", Quantity::share(self.capacity)),
                LabeledValue::new("Charge limit", Quantity::share(self.charge_limit)),
            ]),
        )]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum BatteryTime {
    Idle(IdleBattery),
    Countdown(Countdown),
}

impl BatteryTime {
    fn glyph(self) -> IconGlyph {
        match self {
            BatteryTime::Idle(idle) => idle.icon(),
            BatteryTime::Countdown(_) => Phosphor::HourglassMedium(Weight::Regular).into(),
        }
    }

    fn label_text(self, locale: Locale) -> String {
        match self {
            BatteryTime::Idle(idle) => idle.label().to_string(),
            BatteryTime::Countdown(countdown) => countdown.forecast().label(locale),
        }
    }

    fn display_state(self) -> IndicatorState {
        match self {
            BatteryTime::Countdown(Countdown::UntilEmpty(Forecast::Estimated(estimate)))
                if estimate.time < EMPTY_CRITICAL =>
            {
                IndicatorState::Critical
            }
            BatteryTime::Countdown(Countdown::UntilEmpty(Forecast::Estimated(estimate)))
                if estimate.time < EMPTY_WARNING =>
            {
                IndicatorState::Warning
            }
            BatteryTime::Idle(_) | BatteryTime::Countdown(_) => IndicatorState::Idle,
        }
    }

    pub fn indicator<'indicator>(self, locale: Locale) -> GroupIndicator<'indicator> {
        let label = self.label_text(locale);
        GroupIndicator::glyph_only("Battery time", self.glyph(), self.display_state())
            .with_readout(IndicatorReadout {
                label: ReadoutLabel::Labeled(label.clone()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            })
            .with_alerts(IndicatorAlerts {
                state_toast: StateToast::NotifyFromSeverity {
                    minimum_severity: Severity::Warning,
                    summary_text: "Battery running out",
                    body: ToastBody::Text(label),
                    progress: ToastProgress::Unmeasured,
                },
                ..IndicatorAlerts::default()
            })
            .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(self) -> Vec<Section> {
        match self {
            BatteryTime::Idle(idle) => idle.hover_card(),
            BatteryTime::Countdown(countdown) => {
                vec![Section::new(
                    "Estimate",
                    headed_table(ESTIMATE_HEADINGS, [countdown]),
                )]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use uom::si::ratio::percent;

    use super::*;

    #[test]
    fn indicator_key_names_the_direction() {
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
            .label_text(Locale::en_US)
        };
        assert_eq!(idle(IdleReason::Full), "Full");
        assert_eq!(idle(IdleReason::AtLimit), "At limit");
        assert_eq!(idle(IdleReason::Inhibited), "Paused");
        assert_eq!(idle(IdleReason::Unexplained), "Not charging");
    }
}
