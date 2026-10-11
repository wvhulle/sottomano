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
                ClaudeOrganization, ClaudeUsageWindows,
            },
            copilot::{self, CopilotCcusage, CopilotFree, CopilotMetered},
            openrouter::{self, OpenRouter},
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{polled, polling},
    widgets::{
        continuous_reading::ContinuousReading,
        on_hover_reading::{HoverFetched, OnHoverReading},
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
    available: Available,
    claude_organization: OnHoverReading<ClaudeOrganization>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone, Default)]
pub struct Available {
    claude: Option<ClaudeLogin>,
    ccusage_installed: bool,
    copilot: CopilotAvailable,
}

#[derive(Debug, Clone)]
pub enum ClaudeLogin {
    Subscription(PathBuf),
    UsageBased(PathBuf),
}

impl ClaudeLogin {
    async fn probe(credentials: Option<PathBuf>) -> Option<Self> {
        let credentials = credentials?;
        if ClaudeUsageWindows::available(&credentials).await {
            Some(Self::Subscription(credentials))
        } else if ClaudeEnterpriseSpend::available(&credentials).await {
            Some(Self::UsageBased(credentials))
        } else {
            None
        }
    }

    fn account_subscription(&self) -> Subscription<AiMessage> {
        match self {
            Self::Subscription(credentials) => polling::<ClaudeUsageWindows>(credentials.clone())
                .map(AiMessage::ClaudeUsageWindows),
            Self::UsageBased(credentials) => polling::<ClaudeEnterpriseSpend>(credentials.clone())
                .map(AiMessage::ClaudeEnterpriseSpend),
        }
    }
}

fn claude_subscriptions(
    login: Option<&ClaudeLogin>,
    ccusage_installed: bool,
    working_days: &[Weekday],
) -> Vec<Subscription<AiMessage>> {
    let account = login.map(ClaudeLogin::account_subscription);
    let local_logs = ccusage_installed.then(|| {
        if let Some(ClaudeLogin::Subscription(_)) = login {
            polling::<ClaudeCcusageBlock>(()).map(AiMessage::ClaudeCcusageBlock)
        } else {
            polling::<ClaudeCcusagePeriods>(working_days.to_vec())
                .map(AiMessage::ClaudeCcusagePeriods)
        }
    });
    account.into_iter().chain(local_logs).collect()
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
    ClaudeOrganizationFetched(HoverFetched<ClaudeOrganization>),
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
            openrouter: ContinuousReading::default(),
            claude_usage_windows: ContinuousReading::default(),
            claude_enterprise_spend: ContinuousReading::default(),
            claude_ccusage_periods: ContinuousReading::default(),
            claude_ccusage_block: ContinuousReading::default(),
            copilot_metered: ContinuousReading::default(),
            copilot_free: ContinuousReading::default(),
            copilot_cli: ContinuousReading::default(),
            secrets,
            available: Available::default(),
            claude_organization: OnHoverReading::default(),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn hover(&mut self, hovered: Option<&HoverTarget>, config: &Config) -> Task<AiMessage> {
        self.claude_organization.stop();
        let Some(HoverTarget::Ai(AiTarget::ClaudeEnterpriseSpend)) = hovered else {
            return Task::none();
        };
        let Some(ClaudeLogin::UsageBased(credentials)) = &self.available.claude else {
            return Task::none();
        };
        self.claude_organization.fetch(
            polled::<ClaudeOrganization>(credentials.clone()),
            AiMessage::ClaudeOrganizationFetched,
            config,
        )
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
        subscriptions.extend(claude_subscriptions(
            available.claude.as_ref(),
            available.ccusage_installed,
            working_days,
        ));
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
            AiTarget::ClaudeEnterpriseSpend => {
                self.claude_enterprise_spend.tooltip_with_sections(&[self
                    .claude_organization
                    .section(ClaudeOrganization::sections)])
            }
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
                    .present_group_item(AiTarget::ClaudeUsageWindows),
                self.claude_enterprise_spend
                    .present_group_item(AiTarget::ClaudeEnterpriseSpend),
                self.claude_ccusage_periods
                    .group_item(AiTarget::ClaudeCcusagePeriods),
                self.claude_ccusage_block
                    .present_group_item(AiTarget::ClaudeCcusageBlock),
                self.copilot_metered
                    .present_group_item(AiTarget::CopilotMetered),
                self.copilot_free.group_item(AiTarget::CopilotFree),
                self.copilot_cli.present_group_item(AiTarget::CopilotCli),
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
