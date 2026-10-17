use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClaudeCodeBudgetConfig {
    pub monthly_budget_usd: u32,
}

impl Default for ClaudeCodeBudgetConfig {
    fn default() -> Self {
        Self {
            monthly_budget_usd: 200,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CopilotCliBudgetConfig {
    pub monthly_budget_usd: u32,
}

impl Default for CopilotCliBudgetConfig {
    fn default() -> Self {
        Self {
            monthly_budget_usd: 40,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct OpenRouterBudgetConfig {
    pub monthly_budget_usd: u32,
    pub daily_warning_usd: u32,
}

impl Default for OpenRouterBudgetConfig {
    fn default() -> Self {
        Self {
            monthly_budget_usd: 20,
            daily_warning_usd: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_is_read_per_agent() {
        let config: ClaudeCodeBudgetConfig = toml::from_str("monthly_budget_usd = 100\n").unwrap();
        assert_eq!(config.monthly_budget_usd, 100);
        assert_eq!(CopilotCliBudgetConfig::default().monthly_budget_usd, 40);
    }
}
