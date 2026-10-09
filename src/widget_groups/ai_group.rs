pub mod amount_text;
pub mod claude_code_billing_block;
pub mod claude_code_period_spend;
pub mod claude_enterprise_credit_spend;
pub mod claude_usage_window_gauge;
pub mod copilot_cli_monthly_spend;
pub mod copilot_free_quota_count;
pub mod copilot_quota_gauge;
pub mod openrouter_credit_spend;
pub mod spend_to_level;

use std::path::PathBuf;

use chrono::Weekday;
use iced::{Subscription, Task};

use crate::{
    config::{Config, SecretPaths},
    data::source::{
        Source,
        ai::{
            ccusage,
            claude::{
                self, ClaudeCcusageBlock, ClaudeCcusagePeriods, ClaudeEnterpriseSpend,
                ClaudeUsageWindows,
            },
            copilot::{self, CopilotCcusage, CopilotFree, CopilotMetered},
            openrouter::{self, OpenRouter},
        },
    },
    desktop_shell::{hover_tooltips::HoverTarget, monitor_surfaces::Position},
    message_subscriptions::polling,
    widgets::{
        group_item::GroupItem,
        last_poll::LastPoll,
        phosphor::{Phosphor, Weight},
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, PreviewedValue, WidgetGroup,
        },
    },
};

pub struct AiGroup {
    openrouter: LastPoll<OpenRouter>,
    claude_usage_windows: LastPoll<ClaudeUsageWindows>,
    claude_enterprise_spend: LastPoll<ClaudeEnterpriseSpend>,
    claude_ccusage_periods: LastPoll<ClaudeCcusagePeriods>,
    claude_ccusage_block: LastPoll<ClaudeCcusageBlock>,
    copilot_metered: LastPoll<CopilotMetered>,
    copilot_free: LastPoll<CopilotFree>,
    copilot_cli: LastPoll<CopilotCcusage>,
    secrets: SecretPaths,
    available: Available,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone, Default)]
pub struct Available {
    claude: ClaudeLogin,
    ccusage_installed: bool,
    copilot: CopilotAvailable,
}

#[derive(Debug, Clone, Default)]
pub enum ClaudeLogin {
    Subscription(PathBuf),
    UsageBased(PathBuf),
    #[default]
    Absent,
}

impl ClaudeLogin {
    async fn probe(credentials: Option<PathBuf>) -> Self {
        let Some(credentials) = credentials else {
            return Self::Absent;
        };
        if ClaudeUsageWindows::available(&credentials).await {
            Self::Subscription(credentials)
        } else if ClaudeEnterpriseSpend::available(&credentials).await {
            Self::UsageBased(credentials)
        } else {
            Self::Absent
        }
    }

    fn subscriptions(
        &self,
        ccusage_installed: bool,
        working_days: &[Weekday],
    ) -> Vec<Subscription<AiMessage>> {
        let account = match self {
            Self::Subscription(credentials) => Some(
                polling::<ClaudeUsageWindows>(credentials.clone())
                    .map(AiMessage::ClaudeUsageWindows),
            ),
            Self::UsageBased(credentials) => Some(
                polling::<ClaudeEnterpriseSpend>(credentials.clone())
                    .map(AiMessage::ClaudeEnterpriseSpend),
            ),
            Self::Absent => None,
        };
        let local_logs = ccusage_installed.then(|| match self {
            Self::Subscription(_) => {
                polling::<ClaudeCcusageBlock>(()).map(AiMessage::ClaudeCcusageBlock)
            }
            Self::UsageBased(_) | Self::Absent => {
                polling::<ClaudeCcusagePeriods>(working_days.to_vec())
                    .map(AiMessage::ClaudeCcusagePeriods)
            }
        });
        account.into_iter().chain(local_logs).collect()
    }
}

async fn available_with<S: Source>(key: Option<&S::Key>) -> bool {
    match key {
        Some(key) => S::available(key).await,
        None => false,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CopilotAvailable {
    metered: bool,
    free: bool,
    cli: bool,
}

#[derive(Debug, Clone)]
pub enum AiMessage {
    Available(Available),
    OpenRouter(Result<OpenRouter, openrouter::Error>),
    ClaudeUsageWindows(Result<ClaudeUsageWindows, claude::Error>),
    ClaudeEnterpriseSpend(Result<ClaudeEnterpriseSpend, claude::Error>),
    ClaudeCcusagePeriods(Result<ClaudeCcusagePeriods, claude::Error>),
    ClaudeCcusageBlock(Result<ClaudeCcusageBlock, claude::Error>),
    CopilotMetered(Result<CopilotMetered, copilot::Error>),
    CopilotFree(Result<CopilotFree, copilot::Error>),
    CopilotCli(Result<CopilotCcusage, copilot::Error>),
    Expansion(ExpansionMessage),
}

impl AiGroup {
    pub fn new(secrets: SecretPaths) -> Self {
        Self {
            openrouter: LastPoll::default(),
            claude_usage_windows: LastPoll::default(),
            claude_enterprise_spend: LastPoll::default(),
            claude_ccusage_periods: LastPoll::default(),
            claude_ccusage_block: LastPoll::default(),
            copilot_metered: LastPoll::default(),
            copilot_free: LastPoll::default(),
            copilot_cli: LastPoll::default(),
            secrets,
            available: Available::default(),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn probe(&self) -> Task<AiMessage> {
        let secrets = self.secrets.clone();
        Task::future(async move {
            let github_token = secrets.gh_auth_token.as_ref();
            AiMessage::Available(Available {
                claude: ClaudeLogin::probe(secrets.claude_code_oauth).await,
                ccusage_installed: ccusage::installed().await,
                copilot: CopilotAvailable {
                    metered: available_with::<CopilotMetered>(github_token).await,
                    free: available_with::<CopilotFree>(github_token).await,
                    cli: CopilotCcusage::available(&()).await,
                },
            })
        })
    }

    pub fn subscription(&self, working_days: &[Weekday]) -> Subscription<AiMessage> {
        let secrets = &self.secrets;
        let available = &self.available;
        let mut subscriptions = Vec::new();
        if let Some(key) = secrets.openrouter_management.clone() {
            subscriptions.push(polling::<OpenRouter>(key).map(AiMessage::OpenRouter));
        }
        subscriptions.extend(
            available
                .claude
                .subscriptions(available.ccusage_installed, working_days),
        );
        if let Some(key) = secrets.gh_auth_token.clone() {
            if available.copilot.metered {
                subscriptions
                    .push(polling::<CopilotMetered>(key.clone()).map(AiMessage::CopilotMetered));
            }
            if available.copilot.free {
                subscriptions.push(polling::<CopilotFree>(key).map(AiMessage::CopilotFree));
            }
        }
        if available.copilot.cli {
            subscriptions.push(polling::<CopilotCcusage>(()).map(AiMessage::CopilotCli));
        }
        Subscription::batch(subscriptions)
    }

    pub fn update(&mut self, message: AiMessage) -> Task<AiMessage> {
        match message {
            AiMessage::Available(available) => self.available = available,
            AiMessage::OpenRouter(reading) => self.openrouter.receive(reading),
            AiMessage::ClaudeUsageWindows(reading) => self.claude_usage_windows.receive(reading),
            AiMessage::ClaudeEnterpriseSpend(reading) => {
                self.claude_enterprise_spend.receive(reading);
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

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::AiOpenRouter => self.openrouter.tooltip(),
            HoverTarget::AiClaudeUsageWindows => self.claude_usage_windows.tooltip(),
            HoverTarget::AiClaudeEnterpriseSpend => self.claude_enterprise_spend.tooltip(),
            HoverTarget::AiClaudeCcusagePeriods => self.claude_ccusage_periods.tooltip(),
            HoverTarget::AiClaudeCcusageBlock => self.claude_ccusage_block.tooltip(),
            HoverTarget::AiCopilotMetered => self.copilot_metered.tooltip(),
            HoverTarget::AiCopilotFree => self.copilot_free.tooltip(),
            HoverTarget::AiCopilotCli => self.copilot_cli.tooltip(),
            _ => None,
        }
    }

    pub fn widget_group<'a, Outer: 'a>(&'a self, config: &'a Config, position: Position, lift: fn(AiMessage) -> Outer) -> WidgetGroup<'a, Outer> {
        let claude_usage_windows = self
            .claude_usage_windows
            .present()
            .map(|widget| GroupItem::reading(widget, HoverTarget::AiClaudeUsageWindows));
        WidgetGroup::new(
            &self.expansion,
            AiMessage::Expansion,
            GroupHandle {
                face: HandleFace::Icon(Phosphor::Robot(Weight::Regular).into()),
                preview: self
                    .claude_usage_windows
                    .present()
                    .map(PreviewedValue::Plain),
                forwards_scroll: false,
            },
            &[
                claude_usage_windows,
                self.claude_enterprise_spend
                    .present()
                    .map(|widget| GroupItem::reading(widget, HoverTarget::AiClaudeEnterpriseSpend)),
                self.claude_ccusage_periods
                    .reading_item(HoverTarget::AiClaudeCcusagePeriods),
                self.claude_ccusage_block
                    .present()
                    .map(|widget| GroupItem::reading(widget, HoverTarget::AiClaudeCcusageBlock)),
                self.copilot_metered
                    .present()
                    .map(|widget| GroupItem::reading(widget, HoverTarget::AiCopilotMetered)),
                self.copilot_free.reading_item(HoverTarget::AiCopilotFree),
                self.copilot_cli
                    .present()
                    .map(|widget| GroupItem::reading(widget, HoverTarget::AiCopilotCli)),
                self.openrouter
                    .present()
                    .map(|widget| GroupItem::reading(widget, HoverTarget::AiOpenRouter)),
            ],
            config,
            position,
            lift,
        )
    }
}
