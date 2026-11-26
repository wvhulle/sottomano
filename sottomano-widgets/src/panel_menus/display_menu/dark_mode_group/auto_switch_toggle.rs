use chrono::{DateTime, Locale, Utc};
use situo_client::GeoCoordinates;
use sottomano_desktop_sources::screen_brightness_color::solar_switch_schedule::{
    ColorMode, ModeSwitch, SolarSwitchSchedule, SunEventOffsets, SunOutlook,
};

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::display_menu::dark_mode_group::sqlite_dark_mode_schedule::{
        AutoSwitchArming, DarkModeSchedule,
    },
    widgets::{
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        value_cells::{LabeledValue, ValueCell},
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

    pub fn indicator(&self, clock: Clock, locale: Locale) -> GroupIndicator<'_> {
        let now = clock.utc();
        let label = match self.status_at(now) {
            ScheduleStatus::Following { upcoming, .. } => {
                ReadoutLabel::Labeled(duration_text(upcoming.switch_time - now, locale))
            }
            ScheduleStatus::Disarmed
            | ScheduleStatus::AwaitingLocationFix
            | ScheduleStatus::NoSunEvents => ReadoutLabel::Unlabeled,
        };
        GroupIndicator::glyph_only(
            "Dark mode auto-switch",
            Phosphor::Clock(Weight::Regular).into(),
            IndicatorState::from(self.saved_schedule.auto_switch_arming),
        )
        .with_readout(IndicatorReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |card_clock| {
            self.schedule_hover_card(card_clock)
        }))
    }

    pub fn schedule_hover_card(&self, clock: Clock) -> Vec<Section> {
        let upcoming = match self.status_at(clock.utc()) {
            ScheduleStatus::Disarmed => return vec![],
            ScheduleStatus::AwaitingLocationFix => {
                return vec![Section::new(
                    "Next switch",
                    dictionary_table([LabeledValue::new("Next", "waiting for a location fix")]),
                )];
            }
            ScheduleStatus::NoSunEvents => {
                return vec![Section::new(
                    "Next switch",
                    dictionary_table([LabeledValue::new("Next", "no sunrise or sunset soon")]),
                )];
            }
            ScheduleStatus::Following { upcoming, .. } => upcoming,
        };
        let offset = self
            .saved_schedule
            .sun_event_offsets
            .offset_for_mode(upcoming.target_mode);
        vec![Section::new(
            "Next switch",
            dictionary_table([
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
            ]),
        )]
    }

    pub fn press_does(&self) -> &'static str {
        match self.saved_schedule.auto_switch_arming {
            AutoSwitchArming::Armed => {
                "stop switching between light and dark at sunrise and sunset"
            }
            AutoSwitchArming::Disarmed => "switch to light at sunrise and to dark at sunset",
        }
    }

    pub fn update_location(&mut self, observer_location: GeoCoordinates) {
        self.observer = Observer::Located(observer_location);
    }

    pub fn toggle_auto_switch(&mut self) {
        self.saved_schedule.auto_switch_arming = self.saved_schedule.auto_switch_arming.toggled();
    }

    pub fn change_sun_event_offsets(&mut self, offsets: SunEventOffsets) {
        self.saved_schedule.sun_event_offsets = offsets;
    }

    pub fn mark_checked(&mut self, now: DateTime<Utc>) {
        self.last_switch_check = now;
    }

    pub fn status_at(&self, now: DateTime<Utc>) -> ScheduleStatus {
        let saved = self.saved_schedule;
        if saved.auto_switch_arming == AutoSwitchArming::Disarmed {
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
