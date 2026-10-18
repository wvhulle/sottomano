mod claude_code_billing_block;
mod claude_code_period_spend;
mod claude_enterprise_credit_spend;
mod claude_usage_window_gauge;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    ai::{
        claude::{
            self, ClaudeCcusageBlock, ClaudeCcusagePeriods, ClaudeEnterpriseSpend, ClaudeLocalLogs,
            ClaudeOrganization, ClaudeUsageWindows,
        },
        working_time_forecast::SpendBudget,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        continuous_reading::ContinuousReading,
        on_hover_reading::OnHoverReading,
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, Section, Tooltip},
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeUsageTarget {
    UsageWindows,
    EnterpriseSpend,
    CcusagePeriods,
    CcusageBlock,
}

impl From<claude::Error> for Section {
    fn from(error: claude::Error) -> Self {
        match error {
            claude::Error::RateLimited(cause) | claude::Error::Fetch(cause) => cause.into(),
            claude::Error::Backoff { remaining, cause } => {
                Section::from(cause).with_entries([KeyValue::new(
                    "Retry in",
                    humantime::format_duration(remaining).to_string(),
                )])
            }
            other => Section::error(other),
        }
    }
}

impl From<ClaudeUsageTarget> for HoverTarget {
    fn from(target: ClaudeUsageTarget) -> Self {
        HoverTarget::ClaudeUsage(target)
    }
}

pub struct ClaudeUsageGroup {
    usage_windows: ContinuousReading<ClaudeUsageWindows>,
    enterprise_spend: ContinuousReading<ClaudeEnterpriseSpend>,
    ccusage_periods: ContinuousReading<ClaudeCcusagePeriods>,
    ccusage_block: ContinuousReading<ClaudeCcusageBlock>,
    oauth_credentials: Option<PathBuf>,
    organization: OnHoverReading<ClaudeOrganization>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ClaudeUsageMessage {
    UsageWindows(Box<SourceDelivery<ClaudeUsageWindows>>),
    EnterpriseSpend(SourceDelivery<ClaudeEnterpriseSpend>),
    OrganizationFetched(SourceDelivery<ClaudeOrganization>),
    CcusagePeriods(SourceDelivery<ClaudeCcusagePeriods>),
    CcusageBlock(SourceDelivery<ClaudeCcusageBlock>),
    Expansion(ExpansionMessage),
}

impl ClaudeUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(oauth_credentials: Option<PathBuf>) -> Self {
        Self {
            usage_windows: ContinuousReading::default(),
            enterprise_spend: ContinuousReading::default(),
            ccusage_periods: ContinuousReading::default(),
            ccusage_block: ContinuousReading::default(),
            oauth_credentials,
            organization: OnHoverReading::default(),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
    ) -> Task<ClaudeUsageMessage> {
        self.organization.stop();
        let Some(HoverTarget::ClaudeUsage(ClaudeUsageTarget::EnterpriseSpend)) = hovered else {
            return Task::none();
        };
        let Some(credentials) = self.oauth_credentials.clone() else {
            return Task::none();
        };
        self.organization.start_hover_fetch(
            credentials,
            ClaudeUsageMessage::OrganizationFetched,
            config,
        )
    }

    fn local_logs(&self, config: &Config) -> ClaudeLocalLogs {
        ClaudeLocalLogs {
            budget: SpendBudget::new(config, config.widget_groups.claude_usage.monthly_budget_usd),
            credentials: self.oauth_credentials.clone(),
        }
    }

    pub fn refresh(&self, target: ClaudeUsageTarget, config: &Config) -> Task<ClaudeUsageMessage> {
        let account = self.oauth_credentials.clone();
        match target {
            ClaudeUsageTarget::UsageWindows => account.map_or_else(Task::none, |key| {
                first_delivery::<ClaudeUsageWindows>(key)
                    .map(|reading| ClaudeUsageMessage::UsageWindows(Box::new(reading)))
            }),
            ClaudeUsageTarget::EnterpriseSpend => account.map_or_else(Task::none, |key| {
                first_delivery::<ClaudeEnterpriseSpend>(key)
                    .map(ClaudeUsageMessage::EnterpriseSpend)
            }),
            ClaudeUsageTarget::CcusagePeriods => {
                first_delivery::<ClaudeCcusagePeriods>(self.local_logs(config))
                    .map(ClaudeUsageMessage::CcusagePeriods)
            }
            ClaudeUsageTarget::CcusageBlock => {
                first_delivery::<ClaudeCcusageBlock>(self.local_logs(config))
                    .map(ClaudeUsageMessage::CcusageBlock)
            }
        }
    }

    pub fn subscription(&self, config: &Config) -> Subscription<ClaudeUsageMessage> {
        let account = self.oauth_credentials.clone().into_iter().flat_map(|key| {
            [
                source_subscription::<ClaudeUsageWindows>(key.clone())
                    .map(|reading| ClaudeUsageMessage::UsageWindows(Box::new(reading))),
                source_subscription::<ClaudeEnterpriseSpend>(key)
                    .map(ClaudeUsageMessage::EnterpriseSpend),
            ]
        });
        let local_logs = self.local_logs(config);
        Subscription::batch(
            account.chain([
                source_subscription::<ClaudeCcusageBlock>(local_logs.clone())
                    .map(ClaudeUsageMessage::CcusageBlock),
                source_subscription::<ClaudeCcusagePeriods>(local_logs)
                    .map(ClaudeUsageMessage::CcusagePeriods),
            ]),
        )
    }

    pub fn update(&mut self, message: ClaudeUsageMessage) -> Task<ClaudeUsageMessage> {
        match message {
            ClaudeUsageMessage::UsageWindows(reading) => {
                self.usage_windows.receive_delivery(*reading);
            }
            ClaudeUsageMessage::EnterpriseSpend(reading) => {
                self.enterprise_spend.receive_delivery(reading);
            }
            ClaudeUsageMessage::OrganizationFetched(fetched) => {
                self.organization.receive_delivery(fetched);
            }
            ClaudeUsageMessage::CcusagePeriods(reading) => {
                self.ccusage_periods.receive_delivery(reading);
            }
            ClaudeUsageMessage::CcusageBlock(reading) => {
                self.ccusage_block.receive_delivery(reading);
            }
            ClaudeUsageMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: ClaudeUsageTarget) -> Option<Tooltip> {
        match target {
            ClaudeUsageTarget::UsageWindows => self.usage_windows.tooltip(),
            ClaudeUsageTarget::EnterpriseSpend => self
                .enterprise_spend
                .tooltip_with_sections(self.organization.sections()),
            ClaudeUsageTarget::CcusagePeriods => self.ccusage_periods.tooltip(),
            ClaudeUsageTarget::CcusageBlock => self.ccusage_block.tooltip(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, ClaudeUsageMessage> {
        GroupContent::new(
            &self.expansion,
            ClaudeUsageMessage::Expansion,
            [
                self.usage_windows
                    .group_item(ClaudeUsageTarget::UsageWindows),
                self.enterprise_spend
                    .group_item(ClaudeUsageTarget::EnterpriseSpend),
                self.ccusage_periods
                    .group_item(ClaudeUsageTarget::CcusagePeriods),
                self.ccusage_block
                    .group_item(ClaudeUsageTarget::CcusageBlock),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: SimpleIcon::Claude.into(),
            handle: GroupHandle::ClaudeUsage,
        })
        .preview(self.usage_windows.display().map(PreviewedValue::Plain))
    }
}
