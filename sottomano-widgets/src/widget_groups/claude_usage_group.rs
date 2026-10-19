mod claude_code_billing_block;
mod claude_code_period_spend;
mod claude_enterprise_credit_spend;
mod claude_usage_window_gauge;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    ai_usage_spend::{
        claude_usage::{
            self, ClaudeCcusageBlock, ClaudeCcusageModels, ClaudeCcusagePeriods,
            ClaudeEnterpriseSpend, ClaudeLocalLogs, ClaudeOrganization, ClaudeUsageWindows,
        },
        working_time_forecast::SpendBudget,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        most_used_model::{ModelSpendRanking, MostUsedModel, MostUsedModelSpend, RankedSpan},
        on_hover_reading::OnHoverReading,
        simple_icons::SimpleIcon,
        tooltip_body::{KeyValue, Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeUsageTarget {
    UsageWindows,
    EnterpriseSpend,
    CcusagePeriods,
    CcusageBlock,
    MostUsedModel,
    MostUsedModelSpend,
}

impl From<claude_usage::Error> for Section {
    fn from(error: claude_usage::Error) -> Self {
        match error {
            claude_usage::Error::RateLimited(cause) | claude_usage::Error::Fetch(cause) => {
                cause.into()
            }
            claude_usage::Error::Backoff { remaining, cause } => {
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
    ccusage_models: ContinuousReading<ClaudeCcusageModels>,
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
    CcusageModels(SourceDelivery<ClaudeCcusageModels>),
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
            ccusage_models: ContinuousReading::default(),
            oauth_credentials,
            organization: OnHoverReading::default(),
            expansion: GroupExpansion::default(),
        }
    }

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        clock: Clock,
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
            clock,
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
            ClaudeUsageTarget::MostUsedModel | ClaudeUsageTarget::MostUsedModelSpend => {
                first_delivery::<ClaudeCcusageModels>(self.oauth_credentials.clone())
                    .map(ClaudeUsageMessage::CcusageModels)
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
                source_subscription::<ClaudeCcusageModels>(self.oauth_credentials.clone())
                    .map(ClaudeUsageMessage::CcusageModels),
            ]),
        )
    }

    pub fn update(
        &mut self,
        message: ClaudeUsageMessage,
        clock: Clock,
    ) -> Task<ClaudeUsageMessage> {
        match message {
            ClaudeUsageMessage::UsageWindows(reading) => {
                self.usage_windows.receive_delivery(*reading, clock);
            }
            ClaudeUsageMessage::EnterpriseSpend(reading) => {
                self.enterprise_spend.receive_delivery(reading, clock);
            }
            ClaudeUsageMessage::OrganizationFetched(fetched) => {
                self.organization.receive_delivery(fetched, clock);
            }
            ClaudeUsageMessage::CcusagePeriods(reading) => {
                self.ccusage_periods.receive_delivery(reading, clock);
            }
            ClaudeUsageMessage::CcusageBlock(reading) => {
                self.ccusage_block.receive_delivery(reading, clock);
            }
            ClaudeUsageMessage::CcusageModels(reading) => {
                self.ccusage_models
                    .receive_keeping_last_reading(reading, clock);
            }
            ClaudeUsageMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: ClaudeUsageTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            ClaudeUsageTarget::UsageWindows => self.usage_windows.tooltip(clock),
            ClaudeUsageTarget::EnterpriseSpend => self
                .enterprise_spend
                .tooltip_with_sections(self.organization.sections(clock), clock),
            ClaudeUsageTarget::CcusagePeriods => self.ccusage_periods.tooltip(clock),
            ClaudeUsageTarget::CcusageBlock => self.ccusage_block.tooltip(clock),
            ClaudeUsageTarget::MostUsedModel => self
                .ccusage_models
                .tooltip_with(MostUsedModel(self.model_ranking()?).tooltip(clock)),
            ClaudeUsageTarget::MostUsedModelSpend => self
                .ccusage_models
                .tooltip_with(MostUsedModelSpend(self.billed_model_ranking()?).tooltip(clock)),
        }
    }

    fn billed_model_ranking(&self) -> Option<ModelSpendRanking<'_, 3>> {
        self.ccusage_models
            .get()
            .filter(|models| !models.covered_by_subscription)
            .and(self.model_ranking())
    }

    fn model_ranking(&self) -> Option<ModelSpendRanking<'_, 3>> {
        let models = self.ccusage_models.get()?;
        ModelSpendRanking::new(
            "ccusage, local Claude Code logs, API list prices",
            [
                RankedSpan {
                    heading: "Today",
                    models: &models.today,
                },
                RankedSpan {
                    heading: "Week",
                    models: &models.week,
                },
                RankedSpan {
                    heading: "Month",
                    models: &models.month,
                },
            ],
        )
    }

    pub fn group_content(&self) -> GroupContent<'_, ClaudeUsageMessage> {
        GroupContent::new(
            &self.expansion,
            ClaudeUsageMessage::Expansion,
            [
                self.usage_windows
                    .group_item(ClaudeUsageTarget::UsageWindows)
                    .map(GroupItem::previewed_on_handle),
                self.enterprise_spend
                    .group_item(ClaudeUsageTarget::EnterpriseSpend),
                self.ccusage_periods
                    .group_item(ClaudeUsageTarget::CcusagePeriods),
                self.ccusage_block
                    .group_item(ClaudeUsageTarget::CcusageBlock),
            ]
            .into_iter()
            .flatten()
            .chain(self.model_ranking().map(|ranking| {
                GroupItem::reading(MostUsedModel(ranking), ClaudeUsageTarget::MostUsedModel)
            }))
            .chain(self.billed_model_ranking().map(|ranking| {
                GroupItem::reading(
                    MostUsedModelSpend(ranking),
                    ClaudeUsageTarget::MostUsedModelSpend,
                )
            })),
        )
        .face(HandleFace::Icon {
            glyph: SimpleIcon::Claude.into(),
            handle: GroupHandle::ClaudeUsage,
        })
    }
}
