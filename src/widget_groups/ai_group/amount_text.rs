use chrono::{DateTime, TimeDelta, Utc};

pub fn to_cents(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

pub fn percent_text(value: f64) -> String {
    format!("{}%", to_cents(value))
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
    let remaining = at.signed_duration_since(now).max(TimeDelta::zero());
    let hours = remaining.num_hours();
    let minutes = remaining.num_minutes() - hours * 60;
    format!("{hours}h {minutes}m")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_amounts_keep_up_to_two_decimals() {
        assert_eq!(to_cents(0.2), "0.2");
        assert_eq!(to_cents(199.8), "199.8");
        assert_eq!(to_cents(200.0), "200");
        assert_eq!(to_cents(1.0 / 3.0), "0.33");
        assert_eq!(percent_text(0.1), "0.1%");
    }

    #[test]
    fn token_counts_shorten_to_one_decimal() {
        assert_eq!(token_count(999), "999");
        assert_eq!(token_count(104_769), "104.7k");
        assert_eq!(token_count(19_043_113), "19.0M");
        assert_eq!(token_count(2_500_000_000), "2.5B");
    }
}
