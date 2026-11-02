use chrono::{DateTime, Locale, Utc};
use situo_client::GeoCoordinates;
use sottomano_sources::screen_brightness_color::solar_switch_schedule::{
    ColorMode, ModeSwitch, SolarSwitchSchedule, SunEventOffsets, SunOutlook,
};

use crate::{
    clock::Clock,
    widget_groups::dark_mode_group::sqlite_dark_mode_schedule::DarkModeSchedule,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{
            TooltipBody,
            tooltip_source_delivery::{DeliveryTiming, Tooltip},
        },
        value_cells::{LabeledValue, ValueCell},
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
    Located(GeoCoordinates),
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
            saved_schedule: DarkModeSchedule::default(),
            observer: Observer::AwaitingLocationFix,
            last_switch_check: clock.utc(),
        }
    }

    pub fn schedule_tooltip(&self, clock: Clock) -> Tooltip {
        Tooltip::sourced(self.tooltip(clock), DeliveryTiming::LivePush)
    }

    pub fn press_does(&self) -> &'static str {
        if self.saved_schedule.auto_switch_armed {
            "stop switching between light and dark at sunrise and sunset"
        } else {
            "switch to light at sunrise and to dark at sunset"
        }
    }

    pub fn update_location(&mut self, observer_location: GeoCoordinates) {
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
        let saved = self.saved_schedule;
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

impl GroupItemReading for AutoSwitchToggle {
    fn name(&self) -> &'static str {
        "Dark mode auto-switch"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Clock(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        let now = clock.utc();
        match self.status_at(now) {
            ScheduleStatus::Following { upcoming, .. } => {
                Some(duration_text(upcoming.switch_time - now, locale))
            }
            ScheduleStatus::Disarmed
            | ScheduleStatus::AwaitingLocationFix
            | ScheduleStatus::NoSunEvents => None,
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::of_toggle(self.saved_schedule.auto_switch_armed)
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let upcoming = match self.status_at(clock.utc()) {
            ScheduleStatus::Disarmed => return TooltipBody::sections([]),
            ScheduleStatus::AwaitingLocationFix => {
                return TooltipBody::dictionary([LabeledValue::new(
                    "Next",
                    "waiting for a location fix",
                )]);
            }
            ScheduleStatus::NoSunEvents => {
                return TooltipBody::dictionary([LabeledValue::new(
                    "Next",
                    "no sunrise or sunset soon",
                )]);
            }
            ScheduleStatus::Following { upcoming, .. } => upcoming,
        };
        let offset = self
            .saved_schedule
            .sun_event_offsets
            .offset_for_mode(upcoming.target_mode);
        TooltipBody::dictionary([
            LabeledValue::new(
                upcoming.target_mode.sun_event_label().to_owned(),
                ValueCell::Time {
                    at: upcoming.switch_time - offset,
                    format: LocalizedTimeFormat::TimeOfDay,
                },
            ),
            LabeledValue::new("Offset".to_owned(), ValueCell::SignedDuration(offset)),
            LabeledValue::new(
                format!("Switch to {}", upcoming.target_mode.umbra_name()),
                ValueCell::Time {
                    at: upcoming.switch_time,
                    format: LocalizedTimeFormat::TimeOfDay,
                },
            ),
        ])
    }
}
