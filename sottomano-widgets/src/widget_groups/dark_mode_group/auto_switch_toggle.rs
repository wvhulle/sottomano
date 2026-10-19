use chrono::{DateTime, Locale, TimeDelta, Utc};
use humantime::format_duration;
use situo_client::Position;
use sottomano_sources::screen_brightness_color::solar_switch_schedule::{
    ColorMode, ModeSwitch, SolarSwitchSchedule, SunEventOffsets, SunOutlook,
};
use sottomano_state::dark_mode::DarkModeSchedule;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        continuous_reading::ContinuousReading,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Tooltip, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct AutoSwitchToggle {
    pub saved_schedule: ContinuousReading<DarkModeSchedule>,
    observer: Observer,
    last_switch_check: DateTime<Utc>,
}

enum Observer {
    AwaitingLocationFix,
    Located(Position),
}

pub enum ScheduleStatus {
    Disarmed,
    AwaitingLocationFix,
    NoSunEvents,
    Following {
        latest: ModeSwitch,
        upcoming: ModeSwitch,
    },
}

pub enum SwitchDecision {
    Apply(ColorMode),
    Hold,
}

impl AutoSwitchToggle {
    pub fn new(clock: Clock) -> Self {
        Self {
            saved_schedule: ContinuousReading::default(),
            observer: Observer::AwaitingLocationFix,
            last_switch_check: clock.utc(),
        }
    }

    pub fn schedule_tooltip(&self, clock: Clock) -> Option<Tooltip> {
        self.saved_schedule.tooltip_with(self.tooltip(clock))
    }

    pub fn press_does(&self) -> &'static str {
        if self
            .saved_schedule
            .get()
            .copied()
            .unwrap_or_default()
            .auto_switch_armed
        {
            "stop switching between light and dark at sunrise and sunset"
        } else {
            "switch to light at sunrise and to dark at sunset"
        }
    }

    pub fn update_location(&mut self, observer_location: Position) {
        self.observer = Observer::Located(observer_location);
    }

    pub fn toggle_auto_switch(&mut self, clock: Clock) {
        let mut schedule = self.saved_schedule.get().copied().unwrap_or_default();
        schedule.auto_switch_armed = !schedule.auto_switch_armed;
        self.saved_schedule.load(schedule, clock);
    }

    pub fn change_sun_event_offsets(&mut self, offsets: SunEventOffsets, clock: Clock) {
        let mut schedule = self.saved_schedule.get().copied().unwrap_or_default();
        schedule.sun_event_offsets = offsets;
        self.saved_schedule.load(schedule, clock);
    }

    pub fn mark_checked(&mut self, now: DateTime<Utc>) {
        self.last_switch_check = now;
    }

    pub fn status_at(&self, now: DateTime<Utc>) -> ScheduleStatus {
        let saved = self.saved_schedule.get().copied().unwrap_or_default();
        if !saved.auto_switch_armed {
            return ScheduleStatus::Disarmed;
        }
        let Observer::Located(observer_location) = self.observer else {
            return ScheduleStatus::AwaitingLocationFix;
        };
        let schedule = SolarSwitchSchedule {
            observer_location,
            sun_event_offsets: saved.sun_event_offsets,
        };
        match schedule.outlook_at(now) {
            SunOutlook::NoSunEvents => ScheduleStatus::NoSunEvents,
            SunOutlook::Alternating { latest, upcoming } => {
                ScheduleStatus::Following { latest, upcoming }
            }
        }
    }

    pub fn switch_decision_at(&self, now: DateTime<Utc>) -> SwitchDecision {
        match self.status_at(now) {
            ScheduleStatus::Following { latest, .. }
                if self.last_switch_check < latest.switch_time =>
            {
                SwitchDecision::Apply(latest.target_mode)
            }
            ScheduleStatus::Following { .. }
            | ScheduleStatus::Disarmed
            | ScheduleStatus::AwaitingLocationFix
            | ScheduleStatus::NoSunEvents => SwitchDecision::Hold,
        }
    }
}

pub fn format_offset_minutes(offset: TimeDelta) -> String {
    format!("{:+}m", offset.num_minutes())
}

impl BarWidget for AutoSwitchToggle {
    fn name(&self) -> &'static str {
        "Dark mode auto-switch"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Clock(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, _locale: Locale) -> Option<String> {
        let now = clock.utc();
        match self.status_at(now) {
            ScheduleStatus::Following { upcoming, .. } => {
                let whole_minutes = TimeDelta::minutes((upcoming.switch_time - now).num_minutes());
                Some(format_duration(whole_minutes.to_std().unwrap_or_default()).to_string())
            }
            ScheduleStatus::Disarmed
            | ScheduleStatus::AwaitingLocationFix
            | ScheduleStatus::NoSunEvents => None,
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::of_toggle(
            self.saved_schedule
                .get()
                .copied()
                .unwrap_or_default()
                .auto_switch_armed,
        )
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let upcoming = match self.status_at(clock.utc()) {
            ScheduleStatus::Disarmed => return TooltipBody::sections([]),
            ScheduleStatus::AwaitingLocationFix => {
                return TooltipBody::dictionary([KeyValue::new(
                    "Next",
                    "waiting for a location fix",
                )]);
            }
            ScheduleStatus::NoSunEvents => {
                return TooltipBody::dictionary([KeyValue::new(
                    "Next",
                    "no sunrise or sunset soon",
                )]);
            }
            ScheduleStatus::Following { upcoming, .. } => upcoming,
        };
        let offset = self
            .saved_schedule
            .get()
            .copied()
            .unwrap_or_default()
            .sun_event_offsets
            .offset_for_mode(upcoming.target_mode);
        TooltipBody::dictionary([
            KeyValue::new(
                upcoming.target_mode.sun_event_label().to_owned(),
                Cell::Time {
                    at: upcoming.switch_time - offset,
                    format: LocalizedTimeFormat::TimeOfDay,
                },
            ),
            KeyValue::new(
                "Offset".to_owned(),
                Cell::from(format_offset_minutes(offset)),
            ),
            KeyValue::new(
                format!("Switch to {}", upcoming.target_mode.umbra_name()),
                Cell::Time {
                    at: upcoming.switch_time,
                    format: LocalizedTimeFormat::TimeOfDay,
                },
            ),
        ])
    }
}
