mod claude_code_billing_block;
mod claude_code_period_spend;
mod claude_enterprise_credit_spend;
mod claude_usage_window_gauge;

use std::path::PathBuf;

use chrono::WeekdaySet;
use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        claude_usage::{
            self, ClaudeCcusageBlock, ClaudeCcusageModels, ClaudeCcusagePeriods,
            ClaudeEnterpriseAccount, ClaudeEnterpriseSpend, ClaudeLocalLogs, ClaudeOrganization,
            ClaudeUsageWindows,
        },
        working_time_forecast::{SpendBudget, SpendPacing, WorkingTime},
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{HidesFetchFailures, LatestSnapshot},
        most_used_model::{ModelSpendRanking, MostUsedModel, MostUsedModelSpend, RankedSpan},
        simple_icons::SimpleIcon,
        tooltip_body::{Section, tooltip_source_delivery::Tooltip},
        value_cells::{LabeledValue, ValueCell},
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
                Section::from(cause).with_entries([LabeledValue::new(
                    "Retry in",
                    ValueCell::duration(remaining),
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
    usage_windows: LatestSnapshot<ClaudeUsageWindows>,
    enterprise_spend: LatestSnapshot<ClaudeEnterpriseSpend>,
    ccusage_periods: LatestSnapshot<ClaudeCcusagePeriods>,
    ccusage_block: LatestSnapshot<ClaudeCcusageBlock>,
    ccusage_models: LatestSnapshot<ClaudeCcusageModels, HidesFetchFailures>,
    oauth_credentials: Option<PathBuf>,
    organization: HoverFetchedSnapshot<ClaudeOrganization>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ClaudeUsageMessage {
    UsageWindows(Box<SnapshotDelivery<ClaudeUsageWindows>>),
    EnterpriseSpend(SnapshotDelivery<ClaudeEnterpriseSpend>),
    OrganizationFetched(SnapshotDelivery<ClaudeOrganization>),
    CcusagePeriods(SnapshotDelivery<ClaudeCcusagePeriods>),
    CcusageBlock(SnapshotDelivery<ClaudeCcusageBlock>),
    CcusageModels(SnapshotDelivery<ClaudeCcusageModels>),
    Expansion(ExpansionMessage),
}

impl ClaudeUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(oauth_credentials: Option<PathBuf>) -> Self {
        Self {
            usage_windows: LatestSnapshot::default(),
            enterprise_spend: LatestSnapshot::default(),
            ccusage_periods: LatestSnapshot::default(),
            ccusage_block: LatestSnapshot::default(),
            ccusage_models: LatestSnapshot::default(),
            oauth_credentials,
            organization: HoverFetchedSnapshot::default(),
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
            budget: SpendBudget {
                pacing: SpendPacing::CalendarTime,
                monthly_limit_usd: config.widget_groups.claude_usage.monthly_budget_usd,
            },
            credentials: self.oauth_credentials.clone(),
        }
    }

    pub fn refresh(
        &self,
        target: ClaudeUsageTarget,
        config: &Config,
        working_days: WeekdaySet,
    ) -> Task<ClaudeUsageMessage> {
        let account = self.oauth_credentials.clone();
        match target {
            ClaudeUsageTarget::UsageWindows => account.map_or_else(Task::none, |key| {
                first_delivery::<ClaudeUsageWindows>(key)
                    .map(|reading| ClaudeUsageMessage::UsageWindows(Box::new(reading)))
            }),
            ClaudeUsageTarget::EnterpriseSpend => account.map_or_else(Task::none, |key| {
                first_delivery::<ClaudeEnterpriseSpend>(enterprise_account(
                    key,
                    config,
                    working_days,
                ))
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

    pub fn subscription(
        &self,
        config: &Config,
        working_days: WeekdaySet,
    ) -> Subscription<ClaudeUsageMessage> {
        let account = self.oauth_credentials.clone().into_iter().flat_map(|key| {
            [
                snapshot_subscription::<ClaudeUsageWindows>(key.clone())
                    .map(|reading| ClaudeUsageMessage::UsageWindows(Box::new(reading))),
                snapshot_subscription::<ClaudeEnterpriseSpend>(enterprise_account(
                    key,
                    config,
                    working_days,
                ))
                .map(ClaudeUsageMessage::EnterpriseSpend),
            ]
        });
        let local_logs = self.local_logs(config);
        Subscription::batch(
            account.chain([
                snapshot_subscription::<ClaudeCcusageBlock>(local_logs.clone())
                    .map(ClaudeUsageMessage::CcusageBlock),
                snapshot_subscription::<ClaudeCcusagePeriods>(local_logs)
                    .map(ClaudeUsageMessage::CcusagePeriods),
                snapshot_subscription::<ClaudeCcusageModels>(self.oauth_credentials.clone())
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
                self.usage_windows.receive(*reading, clock);
            }
            ClaudeUsageMessage::EnterpriseSpend(reading) => {
                self.enterprise_spend.receive(reading, clock);
            }
            ClaudeUsageMessage::OrganizationFetched(fetched) => {
                self.organization.receive(fetched, clock);
            }
            ClaudeUsageMessage::CcusagePeriods(reading) => {
                self.ccusage_periods.receive(reading, clock);
            }
            ClaudeUsageMessage::CcusageBlock(reading) => {
                self.ccusage_block.receive(reading, clock);
            }
            ClaudeUsageMessage::CcusageModels(reading) => {
                self.ccusage_models.receive(reading, clock);
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
                .tooltip_with_groups(self.organization.sections(clock), clock),
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

fn enterprise_account(
    credentials: PathBuf,
    config: &Config,
    working_days: WeekdaySet,
) -> ClaudeEnterpriseAccount {
    ClaudeEnterpriseAccount {
        credentials,
        working_time: WorkingTime::new(config, working_days),
    }
}
