use chrono::{DateTime, Days, Local, NaiveDate, NaiveTime, TimeDelta, TimeZone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Active,
    Inactive,
}

impl Phase {
    #[must_use]
    pub fn is_quiet(self) -> bool {
        self == Phase::Active
    }

    #[must_use]
    pub fn of_quiet(quiet: bool) -> Self {
        if quiet {
            Phase::Active
        } else {
            Phase::Inactive
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuietWindow {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl QuietWindow {
    #[must_use]
    pub fn contains(&self, clock: NaiveTime) -> bool {
        if self.start <= self.end {
            self.start <= clock && clock < self.end
        } else {
            clock >= self.start || clock < self.end
        }
    }

    #[must_use]
    pub fn phase_at(&self, now: DateTime<Local>) -> Phase {
        Phase::of_quiet(self.contains(now.time()))
    }

    #[must_use]
    pub fn next_transition_into(&self, phase: Phase, now: DateTime<Local>) -> DateTime<Local> {
        let edge = match phase {
            Phase::Active => self.start,
            Phase::Inactive => self.end,
        };
        let today = now.date_naive();
        let date = if edge > now.time() {
            today
        } else {
            today.checked_add_days(Days::new(1)).unwrap_or(today)
        };
        local_at(date, edge, now)
    }

    #[must_use]
    pub fn next_edge(&self, now: DateTime<Local>) -> DateTime<Local> {
        let upcoming = match self.phase_at(now) {
            Phase::Active => Phase::Inactive,
            Phase::Inactive => Phase::Active,
        };
        self.next_transition_into(upcoming, now)
    }
}

fn local_at(date: NaiveDate, time: NaiveTime, fallback: DateTime<Local>) -> DateTime<Local> {
    let naive = date.and_time(time);
    Local
        .from_local_datetime(&naive)
        .earliest()
        .or_else(|| {
            Local
                .from_local_datetime(&(naive + TimeDelta::hours(1)))
                .earliest()
        })
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use chrono::{Local, NaiveTime, TimeZone};

    use super::{Phase, QuietWindow};

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn overnight() -> QuietWindow {
        QuietWindow {
            start: at(23, 0),
            end: at(7, 0),
        }
    }

    #[test]
    fn overnight_window_contains_both_sides_of_midnight() {
        let window = overnight();
        assert!(window.contains(at(23, 30)));
        assert!(window.contains(at(2, 0)));
        assert!(window.contains(at(23, 0)));
        assert!(!window.contains(at(7, 0)));
        assert!(!window.contains(at(12, 0)));
    }

    #[test]
    fn daytime_window_is_half_open() {
        let window = QuietWindow {
            start: at(13, 0),
            end: at(14, 0),
        };
        assert!(window.contains(at(13, 0)));
        assert!(window.contains(at(13, 59)));
        assert!(!window.contains(at(14, 0)));
        assert!(!window.contains(at(12, 59)));
    }

    #[test]
    fn next_edge_from_inside_is_the_end_on_the_following_morning() {
        let now = Local.with_ymd_and_hms(2026, 3, 10, 23, 30, 0).unwrap();
        let edge = overnight().next_edge(now);
        assert_eq!(edge, Local.with_ymd_and_hms(2026, 3, 11, 7, 0, 0).unwrap());
    }

    #[test]
    fn next_edge_from_outside_is_the_start_tonight() {
        let now = Local.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap();
        let edge = overnight().next_edge(now);
        assert_eq!(edge, Local.with_ymd_and_hms(2026, 3, 10, 23, 0, 0).unwrap());
    }

    #[test]
    fn transition_into_inactive_from_outside_skips_to_tomorrow_when_end_has_passed() {
        let now = Local.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap();
        let edge = overnight().next_transition_into(Phase::Inactive, now);
        assert_eq!(edge, Local.with_ymd_and_hms(2026, 3, 11, 7, 0, 0).unwrap());
    }
}
