use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RainAlertsConfig {
    #[serde(with = "humantime_serde")]
    pub lead_time: Duration,
    pub minimum_millimeters_per_hour: f64,
    pub heavy_millimeters_per_hour: f64,
}

impl Default for RainAlertsConfig {
    fn default() -> Self {
        Self {
            lead_time: Duration::from_mins(15),
            minimum_millimeters_per_hour: 0.5,
            heavy_millimeters_per_hour: 4.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_keys_keep_their_defaults() {
        let config: RainAlertsConfig = toml::from_str("lead_time = \"30m\"\n").unwrap();
        assert_eq!(config.lead_time, Duration::from_mins(30));
        assert!((config.heavy_millimeters_per_hour - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<RainAlertsConfig>("drizzle = 0.1\n").is_err());
    }
}
