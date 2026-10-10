use chrono::{DateTime, TimeDelta, Utc};
use humantime::format_duration;
use situo_client::Position;

use crate::{
    data::sun_schedule::{ColorMode, ModeSwitch, SolarSwitchSchedule, SunEventOffsets, SunOutlook},
    state::dark_mode::DarkModeSchedule,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct AutoSwitchToggle {
    pub saved_schedule: DarkModeSchedule,
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

impl Default for AutoSwitchToggle {
    fn default() -> Self {
        Self {
            saved_schedule: DarkModeSchedule::default(),
            observer: Observer::AwaitingLocationFix,
            last_switch_check: Utc::now(),
        }
    }
}

impl AutoSwitchToggle {
    pub fn update_location(&mut self, observer_location: Position) {
        self.observer = Observer::Located(observer_location);
    }

    pub fn toggle_auto_switch(&mut self) {
        self.saved_schedule.auto_switch_armed = !self.saved_schedule.auto_switch_armed;
    }

    pub fn change_sun_event_offsets(&mut self, offsets: SunEventOffsets) {
        self.saved_schedule.sun_event_offsets = offsets;
    }

    pub fn mark_checked(&mut self, now: DateTime<Utc>) {
        self.last_switch_check = now;
    }

    pub fn status_at(&self, now: DateTime<Utc>) -> ScheduleStatus {
        if !self.saved_schedule.auto_switch_armed {
            return ScheduleStatus::Disarmed;
        }
        let Observer::Located(observer_location) = self.observer else {
            return ScheduleStatus::AwaitingLocationFix;
        };
        let schedule = SolarSwitchSchedule {
            observer_location,
            sun_event_offsets: self.saved_schedule.sun_event_offsets,
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
    fn icon(&self) -> Glyph {
        Phosphor::Clock(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        let now = Utc::now();
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

    fn state(&self) -> WidgetState {
        WidgetState::of_toggle(self.saved_schedule.auto_switch_armed)
    }

    fn tooltip(&self) -> TooltipBody {
        let upcoming = match self.status_at(Utc::now()) {
            ScheduleStatus::Disarmed => {
                return TooltipBody::sections(&[Section::untitled_dictionary([KeyValue::new(
                    "Auto-switch",
                    "off",
                )])]);
            }
            ScheduleStatus::AwaitingLocationFix => {
                return TooltipBody::sections(&[Section::untitled_dictionary([
                    KeyValue::new("Auto-switch", "on"),
                    KeyValue::new("Next", "waiting for a location fix"),
                ])]);
            }
            ScheduleStatus::NoSunEvents => {
                return TooltipBody::sections(&[Section::untitled_dictionary([
                    KeyValue::new("Auto-switch", "on"),
                    KeyValue::new("Next", "no sunrise or sunset soon"),
                ])]);
            }
            ScheduleStatus::Following { upcoming, .. } => upcoming,
        };
        let offset = self
            .saved_schedule
            .sun_event_offsets
            .offset_for_mode(upcoming.target_mode);
        TooltipBody::sections(&[Section::untitled_dictionary([
            KeyValue::new("Auto-switch".to_owned(), Cell::from("on")),
            KeyValue::new(
                upcoming.target_mode.sun_event_label().to_owned(),
                Cell::Time {
                    at: upcoming.switch_time - offset,
                    part: TimePart::TimeOfDay,
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
                    part: TimePart::TimeOfDay,
                },
            ),
        ])])
    }
}
