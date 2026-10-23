use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;
use rusty_money::{Formatter, LocalFormat, Params, Position};
use sottomano_sources::ai_usage_spend::spend_money::Money;
use uom::si::{f64::Ratio, ratio::percent};

const SYMBOL_FIRST: &[Position] = &[Position::Sign, Position::Symbol, Position::Amount];
const SYMBOL_LAST: &[Position] = &[Position::Sign, Position::Amount, Position::Symbol];
const SECONDS_PER_DAY: u64 = 86_400;

#[must_use]
pub fn two_decimals(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0 + 0.0)
}

#[must_use]
pub fn percent_text(share: Ratio) -> String {
    format!("{}%", two_decimals(share.get::<percent>()))
}

#[must_use]
pub fn spend_text(money: &Money) -> String {
    let currency = money.currency();
    let amount = money.amount().normalize();
    let shown_digits = (currency.exponent..=Decimal::MAX_SCALE)
        .find(|digits| !amount.round_dp(*digits).is_zero())
        .filter(|first_significant| *first_significant > currency.exponent)
        .map_or(currency.exponent, |first_significant| first_significant + 1);
    let format = LocalFormat::from_locale(currency.locale);
    Formatter::money(
        &Money::from_decimal(amount, currency),
        Params {
            digit_separator: format.digit_separator,
            exponent_separator: format.exponent_separator,
            separator_pattern: format.digit_separator_pattern,
            positions: if currency.symbol_first {
                SYMBOL_FIRST
            } else {
                SYMBOL_LAST
            },
            rounding: Some(shown_digits),
            symbol: Some(currency.symbol),
            code: Some(currency.iso_alpha_code),
        },
    )
}

#[must_use]
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

#[must_use]
pub fn countdown(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    duration_text(at.signed_duration_since(now).max(TimeDelta::zero()))
}

#[must_use]
pub fn duration_text(span: TimeDelta) -> String {
    let whole_seconds = Duration::from_secs(span.num_seconds().max(0).unsigned_abs());
    humantime::format_duration(whole_seconds).to_string()
}

#[must_use]
pub fn coarse_duration_text(span: TimeDelta) -> String {
    let seconds = span.num_seconds().max(0).unsigned_abs();
    let shown = if seconds < 60 {
        seconds
    } else {
        seconds - seconds % 60
    };
    let days = shown / SECONDS_PER_DAY;
    let within_day = humantime::format_duration(Duration::from_secs(shown % SECONDS_PER_DAY));
    let day_unit = if days == 1 { "day" } else { "days" };
    match (days, shown % SECONDS_PER_DAY) {
        (0, _) => within_day.to_string(),
        (_, 0) => format!("{days}{day_unit}"),
        _ => format!("{days}{day_unit} {within_day}"),
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::ai_usage_spend::spend_money::usd;

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
    fn coarse_durations_count_whole_days_instead_of_months() {
        assert_eq!(coarse_duration_text(TimeDelta::hours(745)), "31days 1h");
        assert_eq!(coarse_duration_text(TimeDelta::days(1)), "1day");
        assert_eq!(
            coarse_duration_text(TimeDelta::minutes(23 * 1440 + 9 * 60 + 18)),
            "23days 9h 18m"
        );
    }

    #[test]
    fn fractional_amounts_keep_up_to_two_decimals() {
        assert_eq!(two_decimals(0.2), "0.2");
        assert_eq!(two_decimals(199.8), "199.8");
        assert_eq!(two_decimals(200.0), "200");
        assert_eq!(two_decimals(1.0 / 3.0), "0.33");
        assert_eq!(percent_text(Ratio::new::<percent>(0.1)), "0.1%");
        assert_eq!(two_decimals(-0.000_4), "0");
    }

    #[test]
    fn spend_shows_cents_and_two_significant_digits_below_a_cent() {
        let spend = |amount: Decimal| spend_text(&usd(amount));
        assert_eq!(spend(Decimal::new(310, 2)), "$3.10");
        assert_eq!(spend(Decimal::new(-0, 2)), "$0.00");
        assert_eq!(spend(Decimal::new(42, 4)), "$0.0042");
    }

    #[test]
    fn token_counts_shorten_to_one_decimal() {
        assert_eq!(token_count(999), "999");
        assert_eq!(token_count(104_769), "104.7k");
        assert_eq!(token_count(19_043_113), "19.0M");
        assert_eq!(token_count(2_500_000_000), "2.5B");
    }
}
