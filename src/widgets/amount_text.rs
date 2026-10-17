use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use uom::si::{f64::Ratio, ratio::percent};

pub fn to_cents(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

pub fn percent_text(share: Ratio) -> String {
    format!("{}%", to_cents(share.get::<percent>()))
}

pub fn dollars(value: f64) -> String {
    format!("${value:.2}")
}

pub fn token_count(tokens: u64) -> String {
    match tokens {
        1_000_000_000.. => format!(
            "{}.{}B",
            tokens / 1_000_000_000,
            tokens % 1_000_000_000 / 100_000_000
        ),
        1_000_000.. => format!("{}.{}M", tokens / 1_000_000, tokens % 1_000_000 / 100_000),
        1_000.. => format!("{}.{}k", tokens / 1_000, tokens % 1_000 / 100),
        _ => tokens.to_string(),
    }
}

pub fn countdown(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    duration_text(at.signed_duration_since(now).max(TimeDelta::zero()))
}

pub fn duration_text(span: TimeDelta) -> String {
    let whole_seconds = Duration::from_secs(span.num_seconds().max(0).unsigned_abs());
    humantime::format_duration(whole_seconds).to_string()
}

pub fn coarse_duration_text(span: TimeDelta) -> String {
    let seconds = span.num_seconds().max(0).unsigned_abs();
    let shown = if seconds < 60 {
        seconds
    } else {
        seconds - seconds % 60
    };
    humantime::format_duration(Duration::from_secs(shown)).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_durations_drop_seconds_from_a_minute_on() {
        assert_eq!(coarse_duration_text(TimeDelta::seconds(48)), "48s");
        assert_eq!(coarse_duration_text(TimeDelta::seconds(61)), "1m");
        assert_eq!(
            coarse_duration_text(TimeDelta::seconds(4 * 3600 + 28 * 60 + 9)),
            "4h 28m"
        );
    }

    #[test]
    fn fractional_amounts_keep_up_to_two_decimals() {
        assert_eq!(to_cents(0.2), "0.2");
        assert_eq!(to_cents(199.8), "199.8");
        assert_eq!(to_cents(200.0), "200");
        assert_eq!(to_cents(1.0 / 3.0), "0.33");
        assert_eq!(percent_text(Ratio::new::<percent>(0.1)), "0.1%");
    }

    #[test]
    fn token_counts_shorten_to_one_decimal() {
        assert_eq!(token_count(999), "999");
        assert_eq!(token_count(104_769), "104.7k");
        assert_eq!(token_count(19_043_113), "19.0M");
        assert_eq!(token_count(2_500_000_000), "2.5B");
    }
}
