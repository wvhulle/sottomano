use std::time::Duration;

use chrono::{DateTime, Locale, TimeDelta, Utc};
use fixed_decimal::{Decimal, Sign};
use icu_experimental::{
    duration::{
        Duration as IcuDuration, DurationFormatter, ValidatedDurationFormatterOptions,
        options::{BaseStyle, DurationFormatterOptions, FieldDisplay},
    },
    relativetime::{RelativeTimeFormatter, RelativeTimeFormatterOptions, options::Numeric},
};
use jiff::{RoundMode, Span, SpanRound, Unit};

pub trait LengthOfTime {
    fn elapsed_length(self) -> Duration;
}

impl LengthOfTime for Duration {
    fn elapsed_length(self) -> Duration {
        self
    }
}

impl LengthOfTime for TimeDelta {
    fn elapsed_length(self) -> Duration {
        match self.to_std() {
            Ok(length) => length,
            Err(_) => Duration::ZERO,
        }
    }
}

fn icu_locale(locale: Locale) -> icu_locale_core::Locale {
    let tag = match locale {
        Locale::POSIX => "en".to_owned(),
        other => other.to_string().replace('_', "-"),
    };
    match icu_locale_core::Locale::try_from_str(&tag) {
        Ok(parsed) => parsed,
        Err(_) => icu_locale_core::Locale::UNKNOWN,
    }
}

fn balanced_span(length: Duration, smallest_unit: Unit) -> Result<Span, jiff::Error> {
    let rounding = SpanRound::new()
        .largest(Unit::Day)
        .smallest(smallest_unit)
        .mode(RoundMode::Trunc)
        .days_are_24_hours();
    Span::try_from(length)?.round(rounding)
}

fn largest_unit_count(span: Span) -> (Unit, i64) {
    if span.get_days() != 0 {
        (Unit::Day, i64::from(span.get_days()))
    } else if span.get_hours() != 0 {
        (Unit::Hour, i64::from(span.get_hours()))
    } else if span.get_minutes() != 0 {
        (Unit::Minute, span.get_minutes())
    } else if span.get_seconds() != 0 {
        (Unit::Second, span.get_seconds())
    } else {
        (Unit::Millisecond, span.get_milliseconds())
    }
}

fn next_smaller_unit(unit: Unit) -> Unit {
    match unit {
        Unit::Day => Unit::Hour,
        Unit::Hour => Unit::Minute,
        Unit::Minute => Unit::Second,
        _ => Unit::Millisecond,
    }
}

#[must_use]
pub fn duration_text(length: impl LengthOfTime, locale: Locale) -> String {
    let length = length.elapsed_length();
    let two_largest_units = balanced_span(length, Unit::Millisecond).and_then(|span| {
        let (largest_unit, _) = largest_unit_count(span);
        balanced_span(length, next_smaller_unit(largest_unit))
    });
    match two_largest_units {
        Ok(span) => formatted_span(span, locale),
        Err(error) => error.to_string(),
    }
}

fn formatted_span(span: Span, locale: Locale) -> String {
    let mut options = DurationFormatterOptions::default();
    options.base = BaseStyle::Narrow;
    if span.is_zero() {
        options.second_visibility = Some(FieldDisplay::Always);
    }
    let validated = ValidatedDurationFormatterOptions::validate(options)
        .expect("narrow duration options are valid");
    let formatter = DurationFormatter::try_new((&icu_locale(locale)).into(), validated)
        .expect("compiled ICU data covers every locale through fallback");
    let mut duration = IcuDuration::new();
    duration.days = u64::from(span.get_days().unsigned_abs());
    duration.hours = u64::from(span.get_hours().unsigned_abs());
    duration.minutes = span.get_minutes().unsigned_abs();
    duration.seconds = span.get_seconds().unsigned_abs();
    duration.milliseconds = span.get_milliseconds().unsigned_abs();
    formatter.format(&duration).to_string()
}

#[must_use]
pub fn signed_duration_text(offset: TimeDelta, locale: Locale) -> String {
    let sign = if offset < TimeDelta::zero() {
        '−'
    } else {
        '+'
    };
    format!("{sign}{}", duration_text(offset.abs(), locale))
}

fn relative_text(locale: Locale, unit: Unit, numeric: Numeric, value: Decimal) -> String {
    let preferences = (&icu_locale(locale)).into();
    let options = RelativeTimeFormatterOptions::default().with_numeric(numeric);
    match unit {
        Unit::Day => RelativeTimeFormatter::try_new_long_day(preferences, options),
        Unit::Hour => RelativeTimeFormatter::try_new_short_hour(preferences, options),
        Unit::Minute => RelativeTimeFormatter::try_new_short_minute(preferences, options),
        _ => RelativeTimeFormatter::try_new_short_second(preferences, options),
    }
    .expect("compiled ICU data covers every locale through fallback")
    .format(value)
    .to_string()
}

#[must_use]
pub fn relative_moment_text(at: DateTime<Utc>, now: DateTime<Utc>, locale: Locale) -> String {
    let offset = at - now;
    let (unit, unit_count) = match balanced_span(offset.abs().elapsed_length(), Unit::Second) {
        Ok(span) => largest_unit_count(span),
        Err(error) => return error.to_string(),
    };
    let numeric = if unit_count == 0 {
        Numeric::Auto
    } else {
        Numeric::Always
    };
    let direction = if offset < TimeDelta::zero() {
        Sign::Negative
    } else {
        Sign::None
    };
    relative_text(
        locale,
        unit,
        numeric,
        Decimal::from(unit_count).with_sign(direction),
    )
}

#[must_use]
pub fn relative_day_word(day_offset: i64, locale: Locale) -> Option<String> {
    let named = relative_text(locale, Unit::Day, Numeric::Auto, Decimal::from(day_offset));
    let numeric = relative_text(
        locale,
        Unit::Day,
        Numeric::Always,
        Decimal::from(day_offset),
    );
    let mut characters = named.chars();
    let first = characters.next()?;
    (named != numeric).then(|| first.to_uppercase().chain(characters).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_day_words_follow_the_locale() {
        assert_eq!(
            relative_day_word(1, Locale::en_US).as_deref(),
            Some("Tomorrow")
        );
        assert_eq!(
            relative_day_word(1, Locale::nl_BE).as_deref(),
            Some("Morgen")
        );
        assert_eq!(
            relative_day_word(2, Locale::nl_BE).as_deref(),
            Some("Overmorgen")
        );
        assert_eq!(relative_day_word(3, Locale::en_US), None);
    }

    #[test]
    fn moments_use_the_largest_whole_unit() {
        let now = DateTime::from_timestamp(1_000_000, 0).unwrap();
        let text = |offset| relative_moment_text(now + offset, now, Locale::en_US);
        assert_eq!(text(TimeDelta::zero()), "now");
        assert_eq!(text(TimeDelta::seconds(-90)), "1 min. ago");
        assert_eq!(text(TimeDelta::hours(3)), "in 3 hr.");
    }

    #[test]
    fn durations_show_their_two_largest_units() {
        let text = |length| duration_text(length, Locale::en_US);
        assert_eq!(text(TimeDelta::zero()), "0s");
        assert_eq!(text(TimeDelta::seconds(-5)), "0s");
        assert_eq!(text(TimeDelta::milliseconds(412)), "412ms");
        assert_eq!(text(TimeDelta::milliseconds(1_250)), "1s 250ms");
        assert_eq!(text(TimeDelta::seconds(48)), "48s");
        assert_eq!(text(TimeDelta::seconds(4 * 3600 + 28 * 60 + 9)), "4h 28m");
        assert_eq!(text(TimeDelta::hours(745)), "31d 1h");
    }
}
