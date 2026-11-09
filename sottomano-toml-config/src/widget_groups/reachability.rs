use std::net::{IpAddr, Ipv4Addr};

use serde::{Deserialize, Serialize};

const LAST_DAY_IN_EVERY_MONTH: u8 = 28;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReachabilityConfig {
    pub latency_target: IpAddr,
    pub metered_cycle_start_day: u8,
}

impl Default for ReachabilityConfig {
    fn default() -> Self {
        Self {
            latency_target: IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            metered_cycle_start_day: 1,
        }
    }
}

impl ReachabilityConfig {
    #[must_use]
    pub fn cycle_start_day(&self) -> u32 {
        u32::from(
            self.metered_cycle_start_day
                .clamp(1, LAST_DAY_IN_EVERY_MONTH),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lone_latency_target_parses() {
        let config: ReachabilityConfig = toml::from_str("latency_target = \"9.9.9.9\"\n").unwrap();
        assert_eq!(config.latency_target, IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)));
    }

    #[test]
    fn cycle_start_day_stays_within_every_month() {
        let config = ReachabilityConfig {
            metered_cycle_start_day: 31,
            ..ReachabilityConfig::default()
        };
        assert_eq!(config.cycle_start_day(), 28);
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<ReachabilityConfig>("ping = true\n").is_err());
    }
}
