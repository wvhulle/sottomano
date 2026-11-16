use chrono::TimeDelta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RainAlertsConfig {
    #[serde(with = "crate::humantime_time_delta")]
    pub lead_time: TimeDelta,
    pub minimum_millimeters_per_hour: f64,
    pub heavy_millimeters_per_hour: f64,
}

impl Default for RainAlertsConfig {
    fn default() -> Self {
        Self {
            lead_time: TimeDelta::minutes(15),
            minimum_millimeters_per_hour: 0.5,
            heavy_millimeters_per_hour: 4.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lone_lead_time_parses() {
        let config: RainAlertsConfig = toml::from_str("lead_time = \"30m\"\n").unwrap();
        assert_eq!(config.lead_time, TimeDelta::minutes(30));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<RainAlertsConfig>("drizzle = 0.1\n").is_err());
    }
}
