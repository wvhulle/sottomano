use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AiUsageConfig {
    pub monthly_budget_usd: MonthlyBudgets,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MonthlyBudgets {
    pub claude_code: Option<u32>,
    pub copilot_cli: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_are_read_per_agent() {
        let config: AiUsageConfig =
            toml::from_str("[monthly_budget_usd]\nclaude_code = 100\n").unwrap();
        assert_eq!(config.monthly_budget_usd.claude_code, Some(100));
        assert_eq!(config.monthly_budget_usd.copilot_cli, None);
    }
}
