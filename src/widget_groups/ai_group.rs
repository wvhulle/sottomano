pub mod amount_text;
mod claude_code_billing_block;
mod claude_code_period_spend;
mod claude_enterprise_credit_spend;
mod claude_usage_window_gauge;
mod copilot_cli_monthly_spend;
mod copilot_free_quota_count;
mod copilot_quota_gauge;
mod openrouter_credit_spend;
mod quota_alert_toast;
mod spend_to_level;

use iced::{Subscription, Task};

use crate::{
    config::{Config, SecretPaths},
    data::source::{
        Delivery,
        ai::{
            ccusage::SpendBudget,
            claude::{
                ClaudeCcusageBlock, ClaudeCcusagePeriods, ClaudeEnterpriseSpend, ClaudeLocalLogs,
                ClaudeOrganization, ClaudeUsageWindows,
            },
            copilot::{CopilotCcusage, CopilotFree, CopilotMetered},
            openrouter::OpenRouter,
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{ForcedRefresh, polling},
    widgets::{
        continuous_reading::ContinuousReading,
        on_hover_reading::OnHoverReading,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiTarget {
    OpenRouter,
    ClaudeUsageWindows,
    ClaudeEnterpriseSpend,
    ClaudeCcusagePeriods,
    ClaudeCcusageBlock,
    CopilotMetered,
    CopilotFree,
    CopilotCli,
}

impl From<AiTarget> for HoverTarget {
    fn from(target: AiTarget) -> Self {
        HoverTarget::Ai(target)
    }
}

pub struct AiGroup {
    openrouter: ContinuousReading<OpenRouter>,
    claude_usage_windows: ContinuousReading<ClaudeUsageWindows>,
    claude_enterprise_spend: ContinuousReading<ClaudeEnterpriseSpend>,
    claude_ccusage_periods: ContinuousReading<ClaudeCcusagePeriods>,
    claude_ccusage_block: ContinuousReading<ClaudeCcusageBlock>,
    copilot_metered: ContinuousReading<CopilotMetered>,
    copilot_free: ContinuousReading<CopilotFree>,
    copilot_cli: ContinuousReading<CopilotCcusage>,
    secrets: SecretPaths,
    claude_organization: OnHoverReading<ClaudeOrganization>,
    expansion: GroupExpansion,
}

fn spend_budget(config: &Config, monthly_limit_usd: Option<u32>) -> SpendBudget {
    SpendBudget {
        working_days: config.calendar.working_days.clone(),
        working_hours: config.calendar.working_hours.clone(),
        monthly_limit_usd,
    }
}

#[derive(Debug, Clone)]
pub enum AiMessage {
    OpenRouter(Delivery<OpenRouter>),
    ClaudeUsageWindows(Box<Delivery<ClaudeUsageWindows>>),
    ClaudeEnterpriseSpend(Delivery<ClaudeEnterpriseSpend>),
    ClaudeOrganizationFetched(Delivery<ClaudeOrganization>),
    ClaudeCcusagePeriods(Delivery<ClaudeCcusagePeriods>),
    ClaudeCcusageBlock(Delivery<ClaudeCcusageBlock>),
    CopilotMetered(Delivery<CopilotMetered>),
    CopilotFree(Delivery<CopilotFree>),
    CopilotCli(Delivery<CopilotCcusage>),
    Expansion(ExpansionMessage),
}

impl AiGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn new(secrets: SecretPaths) -> Self {
        Self {
            openrouter: ContinuousReading::default(),
            claude_usage_windows: ContinuousReading::default(),
            claude_enterprise_spend: ContinuousReading::default(),
            claude_ccusage_periods: ContinuousReading::default(),
            claude_ccusage_block: ContinuousReading::default(),
            copilot_metered: ContinuousReading::default(),
            copilot_free: ContinuousReading::default(),
            copilot_cli: ContinuousReading::default(),
            secrets,
            claude_organization: OnHoverReading::default(),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn hover(&mut self, hovered: Option<&HoverTarget>, config: &Config) -> Task<AiMessage> {
        self.claude_organization.stop();
        let Some(HoverTarget::Ai(AiTarget::ClaudeEnterpriseSpend)) = hovered else {
            return Task::none();
        };
        let Some(credentials) = self.secrets.claude_code_oauth.clone() else {
            return Task::none();
        };
        self.claude_organization
            .poll(credentials, AiMessage::ClaudeOrganizationFetched, config)
    }

    pub fn subscription(
        &self,
        config: &Config,
        forced: Option<ForcedRefresh>,
    ) -> Subscription<AiMessage> {
        let secrets = &self.secrets;
        let claude_budget = config.widget_groups.ai_usage.monthly_budget_usd.claude_code;
        let copilot_budget = config.widget_groups.ai_usage.monthly_budget_usd.copilot_cli;
        let openrouter = secrets
            .openrouter_management
            .clone()
            .map(|key| polling::<OpenRouter>(key, forced).map(AiMessage::OpenRouter));
        let claude_account = secrets
            .claude_code_oauth
            .clone()
            .into_iter()
            .flat_map(|key| {
                [
                    polling::<ClaudeUsageWindows>(key.clone(), forced)
                        .map(|reading| AiMessage::ClaudeUsageWindows(Box::new(reading))),
                    polling::<ClaudeEnterpriseSpend>(key, forced)
                        .map(AiMessage::ClaudeEnterpriseSpend),
                ]
            });
        let claude_local_logs = ClaudeLocalLogs {
            budget: spend_budget(config, claude_budget),
            credentials: secrets.claude_code_oauth.clone(),
        };
        let copilot_account = secrets.gh_auth_token.clone().into_iter().flat_map(|key| {
            [
                polling::<CopilotMetered>(key.clone(), forced).map(AiMessage::CopilotMetered),
                polling::<CopilotFree>(key, forced).map(AiMessage::CopilotFree),
            ]
        });
        Subscription::batch(
            openrouter
                .into_iter()
                .chain(claude_account)
                .chain([
                    polling::<ClaudeCcusageBlock>(claude_local_logs.clone(), forced)
                        .map(AiMessage::ClaudeCcusageBlock),
                    polling::<ClaudeCcusagePeriods>(claude_local_logs, forced)
                        .map(AiMessage::ClaudeCcusagePeriods),
                ])
                .chain(copilot_account)
                .chain([
                    polling::<CopilotCcusage>(spend_budget(config, copilot_budget), forced)
                        .map(AiMessage::CopilotCli),
                ]),
        )
    }

    pub fn update(&mut self, message: AiMessage) -> Task<AiMessage> {
        match message {
            AiMessage::OpenRouter(reading) => self.openrouter.receive(reading),
            AiMessage::ClaudeUsageWindows(reading) => self.claude_usage_windows.receive(*reading),
            AiMessage::ClaudeEnterpriseSpend(reading) => {
                self.claude_enterprise_spend.receive(reading);
            }
            AiMessage::ClaudeOrganizationFetched(fetched) => {
                self.claude_organization.receive(fetched);
            }
            AiMessage::ClaudeCcusagePeriods(reading) => {
                self.claude_ccusage_periods.receive(reading);
            }
            AiMessage::ClaudeCcusageBlock(reading) => self.claude_ccusage_block.receive(reading),
            AiMessage::CopilotMetered(reading) => self.copilot_metered.receive(reading),
            AiMessage::CopilotFree(reading) => self.copilot_free.receive(reading),
            AiMessage::CopilotCli(reading) => self.copilot_cli.receive(reading),
            AiMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: AiTarget) -> Option<Tooltip> {
        match target {
            AiTarget::OpenRouter => self.openrouter.tooltip(),
            AiTarget::ClaudeUsageWindows => self.claude_usage_windows.tooltip(),
            AiTarget::ClaudeEnterpriseSpend => self.claude_enterprise_spend.tooltip_with_sections(
                self.claude_organization
                    .sections(ClaudeOrganization::sections),
            ),
            AiTarget::ClaudeCcusagePeriods => self.claude_ccusage_periods.tooltip(),
            AiTarget::ClaudeCcusageBlock => self.claude_ccusage_block.tooltip(),
            AiTarget::CopilotMetered => self.copilot_metered.tooltip(),
            AiTarget::CopilotFree => self.copilot_free.tooltip(),
            AiTarget::CopilotCli => self.copilot_cli.tooltip(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, AiMessage> {
        GroupContent::new(
            &self.expansion,
            AiMessage::Expansion,
            [
                self.claude_usage_windows
                    .present_quota_group_item(AiTarget::ClaudeUsageWindows),
                self.claude_enterprise_spend
                    .present_quota_group_item(AiTarget::ClaudeEnterpriseSpend),
                self.claude_ccusage_periods
                    .quota_group_item(AiTarget::ClaudeCcusagePeriods),
                self.claude_ccusage_block
                    .present_group_item(AiTarget::ClaudeCcusageBlock),
                self.copilot_metered
                    .present_quota_group_item(AiTarget::CopilotMetered),
                self.copilot_free.group_item(AiTarget::CopilotFree),
                self.copilot_cli
                    .present_quota_group_item(AiTarget::CopilotCli),
                self.openrouter.present_group_item(AiTarget::OpenRouter),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon(Phosphor::Robot(Weight::Regular).into()))
        .preview(
            self.claude_usage_windows
                .present()
                .map(PreviewedValue::Plain),
        )
    }
}
