use std::env;

use chrono::{Locale, NaiveTime, TimeDelta, Weekday};
use chrono_tz::{Asia, Tz};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarChoice {
    pub locale: Option<String>,
    pub other_time_zones: Vec<Tz>,
    pub working_days: Vec<Weekday>,
    pub working_hours: WorkingHours,
}

impl Default for CalendarChoice {
    fn default() -> Self {
        Self {
            locale: None,
            other_time_zones: vec![Asia::Shanghai],
            working_days: vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ],
            working_hours: WorkingHours::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(default, deny_unknown_fields)]
pub struct WorkingHours {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl Default for WorkingHours {
    fn default() -> Self {
        Self {
            start: NaiveTime::MIN + TimeDelta::hours(9),
            end: NaiveTime::MIN + TimeDelta::hours(17),
        }
    }
}

impl WorkingHours {
    pub fn elapsed_share(&self, clock: NaiveTime) -> f64 {
        let length = (self.end - self.start).as_seconds_f64();
        if length <= 0.0 {
            return 0.0;
        }
        ((clock - self.start).as_seconds_f64() / length).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone)]
pub struct Calendar {
    pub locale: Locale,
    pub other_time_zones: Vec<Tz>,
    pub working_days: Vec<Weekday>,
    pub working_hours: WorkingHours,
}

pub fn locale_from_environment() -> Locale {
    ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .filter_map(|variable| env::var(variable).ok())
        .find(|value| !value.is_empty())
        .and_then(|value| without_codeset(&value).parse().ok())
        .unwrap_or_default()
}

fn without_codeset(locale: &str) -> String {
    let modifier = locale.find('@').map_or("", |at| &locale[at..]);
    let name = locale.find(['.', '@']).map_or(locale, |end| &locale[..end]);
    format!("{name}{modifier}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codeset_is_dropped_and_modifier_kept() {
        assert_eq!(without_codeset("en_US.UTF-8"), "en_US");
        assert_eq!(without_codeset("aa_ER.UTF-8@saaho"), "aa_ER@saaho");
        assert_eq!(without_codeset("nl_BE"), "nl_BE");
    }

    fn clock(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    #[test]
    fn working_hours_elapse_from_start_to_end() {
        let hours = WorkingHours::default();
        assert!(hours.elapsed_share(clock(8, 59)).abs() < 1e-9);
        assert!((hours.elapsed_share(clock(11, 0)) - 0.25).abs() < 1e-9);
        assert!((hours.elapsed_share(clock(17, 0)) - 1.0).abs() < 1e-9);
        assert!((hours.elapsed_share(clock(23, 0)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn an_empty_working_window_never_elapses() {
        let hours = WorkingHours {
            start: clock(17, 0),
            end: clock(9, 0),
        };
        assert!(hours.elapsed_share(clock(12, 0)).abs() < 1e-9);
    }

    #[test]
    fn stripped_names_parse_as_locales() {
        assert_eq!(
            without_codeset("nl_BE.UTF-8").parse::<Locale>().ok(),
            Some(Locale::nl_BE)
        );
    }
}
