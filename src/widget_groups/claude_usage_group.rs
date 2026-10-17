mod claude_code_billing_block;
mod claude_code_period_spend;
mod claude_enterprise_credit_spend;
mod claude_usage_window_gauge;

use std::path::PathBuf;

use iced::{Subscription, Task};

use crate::{
    config::Config,
    data::source::{
        Delivery,
        ai::{
            claude::{
                ClaudeCcusageBlock, ClaudeCcusagePeriods, ClaudeEnterpriseSpend, ClaudeLocalLogs,
                ClaudeOrganization, ClaudeUsageWindows,
            },
            working_time_forecast::SpendBudget,
        },
    },
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::{ForcedRefresh, polling},
    widgets::{
        continuous_reading::ContinuousReading,
        on_hover_reading::OnHoverReading,
        simple_icons::SimpleIcon,
        tooltip_body::Tooltip,
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
    UsageWindows(Box<Delivery<ClaudeUsageWindows>>),
    EnterpriseSpend(Delivery<ClaudeEnterpriseSpend>),
    OrganizationFetched(Delivery<ClaudeOrganization>),
    CcusagePeriods(Delivery<ClaudeCcusagePeriods>),
    CcusageBlock(Delivery<ClaudeCcusageBlock>),
    Expansion(ExpansionMessage),
}

impl ClaudeUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

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
        self.organization
            .poll(credentials, ClaudeUsageMessage::OrganizationFetched, config)
    }

    pub fn subscription(
        &self,
        config: &Config,
        forced: Option<ForcedRefresh>,
    ) -> Subscription<ClaudeUsageMessage> {
        let account = self.oauth_credentials.clone().into_iter().flat_map(|key| {
            [
                polling::<ClaudeUsageWindows>(key.clone(), forced)
                    .map(|reading| ClaudeUsageMessage::UsageWindows(Box::new(reading))),
                polling::<ClaudeEnterpriseSpend>(key, forced)
                    .map(ClaudeUsageMessage::EnterpriseSpend),
            ]
        });
        let local_logs = ClaudeLocalLogs {
            budget: SpendBudget::new(config, config.widget_groups.claude_usage.monthly_budget_usd),
            credentials: self.oauth_credentials.clone(),
        };
        Subscription::batch(
            account.chain([
                polling::<ClaudeCcusageBlock>(local_logs.clone(), forced)
                    .map(ClaudeUsageMessage::CcusageBlock),
                polling::<ClaudeCcusagePeriods>(local_logs, forced)
                    .map(ClaudeUsageMessage::CcusagePeriods),
            ]),
        )
    }

    pub fn update(&mut self, message: ClaudeUsageMessage) -> Task<ClaudeUsageMessage> {
        match message {
            ClaudeUsageMessage::UsageWindows(reading) => self.usage_windows.receive(*reading),
            ClaudeUsageMessage::EnterpriseSpend(reading) => self.enterprise_spend.receive(reading),
            ClaudeUsageMessage::OrganizationFetched(fetched) => self.organization.receive(fetched),
            ClaudeUsageMessage::CcusagePeriods(reading) => self.ccusage_periods.receive(reading),
            ClaudeUsageMessage::CcusageBlock(reading) => self.ccusage_block.receive(reading),
            ClaudeUsageMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    pub fn tooltip_for(&self, target: ClaudeUsageTarget) -> Option<Tooltip> {
        match target {
            ClaudeUsageTarget::UsageWindows => self.usage_windows.tooltip(),
            ClaudeUsageTarget::EnterpriseSpend => self
                .enterprise_spend
                .tooltip_with_sections(self.organization.sections(ClaudeOrganization::sections)),
            ClaudeUsageTarget::CcusagePeriods => self.ccusage_periods.tooltip(),
            ClaudeUsageTarget::CcusageBlock => self.ccusage_block.tooltip(),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, ClaudeUsageMessage> {
        let expand = ClaudeUsageMessage::Expansion(ExpansionMessage::Expand);
        GroupContent::new(
            &self.expansion,
            ClaudeUsageMessage::Expansion,
            [
                self.usage_windows
                    .present_quota_group_item(ClaudeUsageTarget::UsageWindows, expand.clone()),
                self.enterprise_spend
                    .present_quota_group_item(ClaudeUsageTarget::EnterpriseSpend, expand.clone()),
                self.ccusage_periods
                    .quota_group_item(ClaudeUsageTarget::CcusagePeriods, expand),
                self.ccusage_block
                    .present_group_item(ClaudeUsageTarget::CcusageBlock),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: SimpleIcon::Claude.into(),
            handle: GroupHandle::ClaudeUsage,
        })
        .preview(self.usage_windows.present().map(PreviewedValue::Plain))
    }
}
