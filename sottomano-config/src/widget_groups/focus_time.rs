use std::{collections::BTreeMap, time::Duration};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FocusTimeConfig {
    #[serde(with = "humantime_serde")]
    pub idle_after: Duration,
    #[serde(with = "humantime_serde")]
    pub session_warning: Duration,
    #[serde(with = "humantime_serde")]
    pub session_critical: Duration,
    #[serde(with = "humantime_serde")]
    pub break_grace: Duration,
    pub switches_per_hour_warning: u32,
    pub switches_per_hour_critical: u32,
    #[serde(with = "humantime_serde")]
    pub switch_heat_span: Duration,
    pub switch_heat_warning: u32,
    pub switch_heat_critical: u32,
    pub screen_daily_budget_minutes: Option<u32>,
    pub app_daily_budget_minutes: BTreeMap<String, u32>,
    pub project_daily_budget_minutes: BTreeMap<String, u32>,
}

impl Default for FocusTimeConfig {
    fn default() -> Self {
        Self {
            idle_after: Duration::from_mins(3),
            session_warning: Duration::from_mins(45),
            session_critical: Duration::from_mins(90),
            break_grace: Duration::from_mins(5),
            switches_per_hour_warning: 100,
            switches_per_hour_critical: 300,
            switch_heat_span: Duration::from_mins(30),
            switch_heat_warning: 20,
            switch_heat_critical: 32,
            screen_daily_budget_minutes: None,
            app_daily_budget_minutes: BTreeMap::new(),
            project_daily_budget_minutes: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_are_keyed_by_verbatim_app_id_and_project_path() {
        let config: FocusTimeConfig = toml::from_str(
            "[app_daily_budget_minutes]\nfirefox = \
             120\n[project_daily_budget_minutes]\n\"/home/me/Code/sottomano\" = 240\n",
        )
        .unwrap();
        assert_eq!(config.app_daily_budget_minutes["firefox"], 120);
        assert_eq!(
            config.project_daily_budget_minutes["/home/me/Code/sottomano"],
            240
        );
        assert_eq!(config.idle_after, Duration::from_mins(3));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(toml::from_str::<FocusTimeConfig>("day_starts_at = \"07:00\"\n").is_err());
    }
}
