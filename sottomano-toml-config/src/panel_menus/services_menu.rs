pub mod spend_budget;
pub mod systemd_manager;

use serde::{Deserialize, Serialize};

use self::{
    spend_budget::{ClaudeCodeBudgetConfig, CopilotCliBudgetConfig, OpenRouterBudgetConfig},
    systemd_manager::SystemdManagerConfig,
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServicesMenuConfig {
    pub systemd_manager: SystemdManagerConfig,
    pub claude_usage: ClaudeCodeBudgetConfig,
    pub copilot_usage: CopilotCliBudgetConfig,
    pub openrouter_credit: OpenRouterBudgetConfig,
}
