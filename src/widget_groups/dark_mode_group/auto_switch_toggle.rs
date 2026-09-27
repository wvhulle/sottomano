use chrono::{DateTime, TimeDelta, Utc};
use situo_client::Position;

use crate::{
    data::sun_schedule::{ColorMode, SolarSwitchSchedule, SunEventOffsets},
    state::dark_mode::DarkModeSchedule,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        font_awesome::Solid,
        localized_time::TimePart,
        tooltip_body::{TooltipBody, row, time_row},
    },
};

#[derive(Default)]
pub struct AutoSwitchToggle {
    pub saved_schedule: DarkModeSchedule,
    tracked_sun: Option<TrackedSunMode>,
}

struct TrackedSunMode {
    observer_location: Position,
    sun_mode: ColorMode,
}

impl AutoSwitchToggle {
    pub fn is_awaiting_switch(&self) -> bool {
        self.saved_schedule.auto_switch_armed && self.tracked_sun.is_some()
    }

    pub fn restore_saved_schedule(&mut self, saved: DarkModeSchedule, now: DateTime<Utc>) {
        self.saved_schedule = saved;
        self.detect_mode_crossing(now);
    }

    pub fn update_location(&mut self, observer_location: Position, now: DateTime<Utc>) {
        if self
            .tracked_sun
            .as_ref()
            .is_some_and(|sun| sun.observer_location == observer_location)
        {
            return;
        }
        self.tracked_sun = self
            .solar_schedule_at(observer_location)
            .mode_at_time(now)
            .map(|sun_mode| TrackedSunMode {
                observer_location,
                sun_mode,
            });
    }

    pub fn toggle_auto_switch(&mut self, now: DateTime<Utc>) -> Option<ColorMode> {
        self.saved_schedule.auto_switch_armed = !self.saved_schedule.auto_switch_armed;
        self.detect_mode_crossing(now);
        self.tracked_sun
            .as_ref()
            .filter(|_| self.saved_schedule.auto_switch_armed)
            .map(|sun| sun.sun_mode)
    }

    pub fn change_sun_event_offsets(
        &mut self,
        offsets: SunEventOffsets,
        now: DateTime<Utc>,
    ) -> Option<ColorMode> {
        self.saved_schedule.sun_event_offsets = offsets;
        self.detect_mode_crossing(now)
    }

    pub fn detect_mode_crossing(&mut self, now: DateTime<Utc>) -> Option<ColorMode> {
        let sun_event_offsets = self.saved_schedule.sun_event_offsets;
        let sun = self.tracked_sun.as_mut()?;
        let current = SolarSwitchSchedule {
            observer_location: sun.observer_location,
            sun_event_offsets,
        }
        .mode_at_time(now)?;
        if current == sun.sun_mode {
            return None;
        }
        sun.sun_mode = current;
        self.saved_schedule.auto_switch_armed.then_some(current)
    }

    fn solar_schedule_at(&self, observer_location: Position) -> SolarSwitchSchedule {
        SolarSwitchSchedule {
            observer_location,
            sun_event_offsets: self.saved_schedule.sun_event_offsets,
        }
    }
}

pub fn format_offset_minutes(offset: TimeDelta) -> String {
    format!("{:+}m", offset.num_minutes())
}

fn format_time_until(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let minutes = (at - now).num_minutes().max(0);
    match minutes / 60 {
        0 => format!("in {minutes}m"),
        hours => format!("in {hours}h {}m", minutes % 60),
    }
}

impl BarWidget for AutoSwitchToggle {
    fn icon(&self) -> Glyph {
        Solid::Clock.into()
    }

    fn label(&self) -> Option<String> {
        let sun = self
            .tracked_sun
            .as_ref()
            .filter(|_| self.saved_schedule.auto_switch_armed)?;
        let now = Utc::now();
        let switch = self
            .solar_schedule_at(sun.observer_location)
            .next_switch_after(now)?;
        Some(format_time_until(switch.switch_time, now))
    }

    fn disabled(&self) -> bool {
        !self.saved_schedule.auto_switch_armed
    }

    fn tooltip(&self) -> TooltipBody {
        let auto = row(
            "Auto-switch",
            if self.saved_schedule.auto_switch_armed {
                "on"
            } else {
                "off"
            },
        );
        let Some(switch) = self.tracked_sun.as_ref().and_then(|sun| {
            self.solar_schedule_at(sun.observer_location)
                .next_switch_after(Utc::now())
        }) else {
            return TooltipBody::rows(vec![auto, row("Next", "waiting for a location fix")]);
        };
        let offset = self
            .saved_schedule
            .sun_event_offsets
            .offset_for_mode(switch.target_mode);
        TooltipBody::rows(vec![
            auto,
            time_row(
                switch.target_mode.sun_event_label(),
                switch.switch_time - offset,
                TimePart::TimeOfDay,
            ),
            row("Offset", format_offset_minutes(offset)),
            time_row(
                &format!("Switch to {}", switch.target_mode.umbra_name()),
                switch.switch_time,
                TimePart::TimeOfDay,
            ),
        ])
    }
}
