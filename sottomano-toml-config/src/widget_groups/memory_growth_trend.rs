use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MemoryGrowthTrendConfig {
    #[serde(with = "humantime_serde")]
    pub trend_window: Duration,
    #[serde(with = "humantime_serde")]
    pub warning_when_full_within: Duration,
    #[serde(with = "humantime_serde")]
    pub critical_when_full_within: Duration,
}

impl Default for MemoryGrowthTrendConfig {
    fn default() -> Self {
        Self {
            trend_window: Duration::from_mins(2),
            warning_when_full_within: Duration::from_mins(5),
            critical_when_full_within: Duration::from_mins(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lone_trend_window_parses() {
        let config: MemoryGrowthTrendConfig = toml::from_str("trend_window = \"5m\"\n").unwrap();
        assert_eq!(config.trend_window, Duration::from_mins(5));
        assert_eq!(config.warning_when_full_within, Duration::from_mins(5));
    }
}
