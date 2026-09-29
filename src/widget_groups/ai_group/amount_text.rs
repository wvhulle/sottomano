pub fn to_cents(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

pub fn percent_text(value: f64) -> String {
    format!("{}%", to_cents(value))
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
}
