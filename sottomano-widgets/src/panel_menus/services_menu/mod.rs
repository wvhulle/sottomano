pub mod ci_runs_group;
pub mod claude_usage_group;
pub mod copilot_usage_group;
pub mod journal_events_group;
pub mod llama_server_group;
pub mod openrouter_credit_group;
pub mod pending_upgrades_group;
pub mod systemd_manager_group;

use sottomano_toml_config::{Config, secrets::StoredSecret};

use self::{
    ci_runs_group::CiRunsGroup, claude_usage_group::ClaudeUsageGroup,
    copilot_usage_group::CopilotUsageGroup, journal_events_group::JournalEventsGroup,
    llama_server_group::LlamaServerGroup, openrouter_credit_group::OpenRouterCreditGroup,
    pending_upgrades_group::PendingUpgradesGroup, systemd_manager_group::SystemdManagerGroup,
};
use crate::{clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent};

pub struct ServicesMenu {
    pub llama_server_group: LlamaServerGroup,
    pub ci_runs_group: CiRunsGroup,
    pub claude_usage_group: ClaudeUsageGroup,
    pub copilot_usage_group: CopilotUsageGroup,
    pub openrouter_credit_group: OpenRouterCreditGroup,
    pub systemd_manager_group: SystemdManagerGroup,
    pub journal_events_group: JournalEventsGroup,
    pub pending_upgrades_group: PendingUpgradesGroup,
}

impl ServicesMenu {
    pub fn collapse(&mut self) {
        self.llama_server_group.collapse();
        self.ci_runs_group.collapse();
        self.claude_usage_group.collapse();
        self.copilot_usage_group.collapse();
        self.openrouter_credit_group.collapse();
        self.systemd_manager_group.collapse();
        self.journal_events_group.collapse();
        self.pending_upgrades_group.collapse();
    }

    #[must_use]
    pub fn new(
        claude_code_oauth: StoredSecret,
        gh_auth_token: StoredSecret,
        openrouter_management: StoredSecret,
    ) -> Self {
        Self {
            llama_server_group: LlamaServerGroup::default(),
            ci_runs_group: CiRunsGroup::default(),
            claude_usage_group: ClaudeUsageGroup::new(claude_code_oauth),
            copilot_usage_group: CopilotUsageGroup::new(gh_auth_token),
            openrouter_credit_group: OpenRouterCreditGroup::new(openrouter_management),
            systemd_manager_group: SystemdManagerGroup::default(),
            journal_events_group: JournalEventsGroup::default(),
            pending_upgrades_group: PendingUpgradesGroup::default(),
        }
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        vec![
            self.llama_server_group.group_content(),
            self.ci_runs_group
                .group_content(clock, config.calendar.locale),
            self.claude_usage_group.group_content(clock),
            self.copilot_usage_group.group_content(clock),
            self.openrouter_credit_group.group_content(clock),
            self.systemd_manager_group.group_content(config, clock),
            self.journal_events_group
                .system_journal_content(config, clock),
            self.journal_events_group
                .user_journal_content(config, clock),
            self.pending_upgrades_group.group_content(config),
        ]
    }
}
