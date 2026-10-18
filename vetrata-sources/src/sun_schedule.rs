use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use situo_client::Position;
use sunrise::{Coordinates, SolarDay, SolarEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Light,
    Dark,
}

impl ColorMode {
    #[must_use]
    pub fn umbra_name(self) -> &'static str {
        match self {
            ColorMode::Light => "light",
            ColorMode::Dark => "dark",
        }
    }

    #[must_use]
    pub fn sun_event_label(self) -> &'static str {
        match self {
            ColorMode::Light => "Sunrise",
            ColorMode::Dark => "Sunset",
        }
    }

    #[must_use]
    pub fn opposite_mode(self) -> Self {
        match self {
            ColorMode::Light => ColorMode::Dark,
            ColorMode::Dark => ColorMode::Light,
        }
    }

    fn triggering_solar_event(self) -> SolarEvent {
        match self {
            ColorMode::Light => SolarEvent::Sunrise,
            ColorMode::Dark => SolarEvent::Sunset,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SunEventOffsets {
    pub sunrise_offset: TimeDelta,
    pub sunset_offset: TimeDelta,
}

impl SunEventOffsets {
    #[must_use]
    pub fn offset_for_mode(self, mode: ColorMode) -> TimeDelta {
        match mode {
            ColorMode::Light => self.sunrise_offset,
            ColorMode::Dark => self.sunset_offset,
        }
    }

    #[must_use]
    pub fn with_offset_for_mode(self, mode: ColorMode, offset: TimeDelta) -> Self {
        match mode {
            ColorMode::Light => Self {
                sunrise_offset: offset,
                ..self
            },
            ColorMode::Dark => Self {
                sunset_offset: offset,
                ..self
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeSwitch {
    pub target_mode: ColorMode,
    pub switch_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolarSwitchSchedule {
    pub observer_location: Position,
    pub sun_event_offsets: SunEventOffsets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SunOutlook {
    Alternating {
        latest: ModeSwitch,
        upcoming: ModeSwitch,
    },
    NoSunEvents,
}

impl SolarSwitchSchedule {
    #[must_use]
    pub fn outlook_at(&self, now: DateTime<Utc>) -> SunOutlook {
        let switches = self.switches_around_date(now);
        let (earlier, later) =
            switches.split_at(switches.partition_point(|switch| switch.switch_time <= now));
        let Some(&latest) = earlier.last() else {
            return SunOutlook::NoSunEvents;
        };
        let Some(&upcoming) = later.first() else {
            return SunOutlook::NoSunEvents;
        };
        SunOutlook::Alternating { latest, upcoming }
    }

    fn switches_around_date(&self, now: DateTime<Utc>) -> Vec<ModeSwitch> {
        let Some(coordinates) = Coordinates::new(
            self.observer_location.latitude,
            self.observer_location.longitude,
        ) else {
            return Vec::new();
        };
        let today = now.date_naive();
        let mut switches: Vec<ModeSwitch> = [today.pred_opt(), Some(today), today.succ_opt()]
            .into_iter()
            .flatten()
            .flat_map(|day| self.switches_on_date(coordinates, day))
            .collect();
        switches.sort_by_key(|switch| switch.switch_time);
        switches
    }

    fn switches_on_date(
        &self,
        coordinates: Coordinates,
        day: NaiveDate,
    ) -> impl Iterator<Item = ModeSwitch> {
        let solar_day = SolarDay::new(coordinates, day);
        [ColorMode::Light, ColorMode::Dark]
            .into_iter()
            .filter_map(move |mode| {
                solar_day
                    .event_time(mode.triggering_solar_event())
                    .map(|at| ModeSwitch {
                        target_mode: mode,
                        switch_time: at + self.sun_event_offsets.offset_for_mode(mode),
                    })
            })
    }
}
