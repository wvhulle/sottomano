use crate::widget_groups::claude_usage_group::{
    claude_code_billing_block::{
        CLAUDE_CCUSAGE_BLOCK_FALLBACK_GLYPH, claude_ccusage_block_hover_card,
        claude_ccusage_block_indicator,
    },
    claude_code_period_spend::{
        CLAUDE_CCUSAGE_PERIODS_FALLBACK_GLYPH, claude_ccusage_periods_hover_card,
        claude_ccusage_periods_indicator,
    },
    claude_enterprise_credit_spend::{
        CLAUDE_ENTERPRISE_SPEND_FALLBACK_GLYPH, claude_enterprise_hover_card,
        claude_enterprise_indicator,
    },
    claude_usage_window_gauge::{claude_usage_windows_hover_card, claude_usage_windows_indicator},
};
mod claude_code_billing_block;
mod claude_code_period_spend;
mod claude_enterprise_credit_spend;
mod claude_usage_window_gauge;

use std::path::PathBuf;

use chrono::WeekdaySet;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        claude_usage::{
            ClaudeCcusageBlock, ClaudeCcusageModels, ClaudeCcusagePeriods, ClaudeEnterpriseAccount,
            ClaudeEnterpriseSpend, ClaudeLocalLogs, ClaudeOrganization, ClaudeUsageWindows,
            SubscriptionCoverage,
        },
        working_time_forecast::{SpendBudget, SpendPacing, WorkingTime},
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
    },
    widget_groups::{
        claude_usage_group::claude_enterprise_credit_spend::claude_organization_sections,
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        most_used_model::{ModelSpendRanking, RankedSpan},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeLocalSource {
    CcusagePeriods,
    CcusageBlock,
    CcusageModels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeUsageBusRequest {
    StartOrganizationFetch,
    RefreshUsageWindows,
    RefreshEnterpriseSpend,
}

#[derive(Debug, Clone, Copy)]
pub enum ClaudeUsageInteraction {
    Expansion(WidgetGroupExpansionMessage),
    StopHoverFetches,
    Refresh(ClaudeLocalSource),
    OnBuses(ClaudeUsageBusRequest),
}

pub struct ClaudeUsageGroup {
    usage_windows: SubscribedSnapshot<ClaudeUsageWindows>,
    enterprise_spend: SubscribedSnapshot<ClaudeEnterpriseSpend>,
    ccusage_periods: SubscribedSnapshot<ClaudeCcusagePeriods>,
    ccusage_block: SubscribedSnapshot<ClaudeCcusageBlock>,
    ccusage_models: SubscribedSnapshot<ClaudeCcusageModels>,
    oauth_credentials: Option<PathBuf>,
    organization: HoverFetchedSnapshot<ClaudeOrganization>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum ClaudeUsageMessage {
    UsageWindows(Box<SnapshotDelivery<ClaudeUsageWindows>>),
    EnterpriseSpend(SnapshotDelivery<ClaudeEnterpriseSpend>),
    OrganizationFetched(SnapshotDelivery<ClaudeOrganization>),
    CcusagePeriods(SnapshotDelivery<ClaudeCcusagePeriods>),
    CcusageBlock(SnapshotDelivery<ClaudeCcusageBlock>),
    CcusageModels(SnapshotDelivery<ClaudeCcusageModels>),
}

impl ClaudeUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(oauth_credentials: Option<PathBuf>) -> Self {
        Self {
            usage_windows: SubscribedSnapshot::default(),
            enterprise_spend: SubscribedSnapshot::default(),
            ccusage_periods: SubscribedSnapshot::default(),
            ccusage_block: SubscribedSnapshot::default(),
            ccusage_models: SubscribedSnapshot::default(),
            oauth_credentials,
            organization: HoverFetchedSnapshot::default(),
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn on_buses(
        &mut self,
        request: ClaudeUsageBusRequest,
        config: &Config,
        working_days: WeekdaySet,
        buses: &DesktopBuses,
        clock: Clock,
    ) -> Task<ClaudeUsageMessage> {
        let Some(credentials) = self.oauth_credentials.clone() else {
            return Task::none();
        };
        match request {
            ClaudeUsageBusRequest::StartOrganizationFetch => self.organization.start_hover_fetch(
                OnBus {
                    buses: buses.clone(),
                    inputs: credentials,
                },
                ClaudeUsageMessage::OrganizationFetched,
                config,
                clock,
            ),
            ClaudeUsageBusRequest::RefreshUsageWindows => {
                first_delivery::<ClaudeUsageWindows>(OnBus {
                    buses: buses.clone(),
                    inputs: credentials,
                })
                .map(|sample| ClaudeUsageMessage::UsageWindows(Box::new(sample)))
            }
            ClaudeUsageBusRequest::RefreshEnterpriseSpend => {
                first_delivery::<ClaudeEnterpriseSpend>(enterprise_account(
                    credentials,
                    config,
                    working_days,
                    buses,
                ))
                .map(ClaudeUsageMessage::EnterpriseSpend)
            }
        }
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

    fn refresh(&self, source: ClaudeLocalSource, config: &Config) -> Task<ClaudeUsageMessage> {
        match source {
            ClaudeLocalSource::CcusagePeriods => {
                first_delivery::<ClaudeCcusagePeriods>(self.local_logs(config))
                    .map(ClaudeUsageMessage::CcusagePeriods)
            }
            ClaudeLocalSource::CcusageModels => {
                first_delivery::<ClaudeCcusageModels>(self.oauth_credentials.clone())
                    .map(ClaudeUsageMessage::CcusageModels)
            }
            ClaudeLocalSource::CcusageBlock => {
                first_delivery::<ClaudeCcusageBlock>(self.local_logs(config))
                    .map(ClaudeUsageMessage::CcusageBlock)
            }
        }
    }

    pub fn subscription(
        &self,
        config: &Config,
        working_days: WeekdaySet,
        buses: Option<&DesktopBuses>,
    ) -> Subscription<ClaudeUsageMessage> {
        let account = self
            .oauth_credentials
            .clone()
            .zip(buses)
            .into_iter()
            .flat_map(|(key, buses)| {
                [
                    snapshot_subscription::<ClaudeUsageWindows>(OnBus {
                        buses: buses.clone(),
                        inputs: key.clone(),
                    })
                    .map(|sample| ClaudeUsageMessage::UsageWindows(Box::new(sample))),
                    snapshot_subscription::<ClaudeEnterpriseSpend>(enterprise_account(
                        key,
                        config,
                        working_days,
                        buses,
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
            ClaudeUsageMessage::UsageWindows(sample) => {
                self.usage_windows.receive(*sample, clock);
            }
            ClaudeUsageMessage::EnterpriseSpend(sample) => {
                self.enterprise_spend.receive(sample, clock);
            }
            ClaudeUsageMessage::OrganizationFetched(fetched) => {
                self.organization.receive(fetched, clock);
            }
            ClaudeUsageMessage::CcusagePeriods(sample) => {
                self.ccusage_periods.receive(sample, clock);
            }
            ClaudeUsageMessage::CcusageBlock(sample) => {
                self.ccusage_block.receive(sample, clock);
            }
            ClaudeUsageMessage::CcusageModels(sample) => {
                self.ccusage_models.receive(sample, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: ClaudeUsageInteraction,
        config: &Config,
    ) -> Task<ClaudeUsageMessage> {
        match interaction {
            ClaudeUsageInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            ClaudeUsageInteraction::StopHoverFetches => {
                self.organization.stop();
                Task::none()
            }
            ClaudeUsageInteraction::Refresh(source) => self.refresh(source, config),
            ClaudeUsageInteraction::OnBuses(_) => Task::none(),
        }
    }

    fn billed_model_ranking(&self) -> Option<ModelSpendRanking<'_, 3>> {
        self.ccusage_models
            .get()
            .filter(|models| models.subscription_coverage == SubscriptionCoverage::Uncovered)
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

    fn enterprise_spend_item<'a>(&'a self, spend_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        spend_row
            .with_hover_card(Box::new(move |_| {
                let mut sections = self
                    .enterprise_spend
                    .sections_described_by(claude_enterprise_hover_card);
                sections.extend(
                    self.organization
                        .sections_described_by(claude_organization_sections),
                );
                sections
            }))
            .fetching_on_hover(
                ClaudeUsageInteraction::OnBuses(ClaudeUsageBusRequest::StartOrganizationFetch),
                ClaudeUsageInteraction::StopHoverFetches,
            )
            .refreshed_by(ClaudeUsageInteraction::OnBuses(
                ClaudeUsageBusRequest::RefreshEnterpriseSpend,
            ))
    }

    fn ccusage_periods_item<'a>(&'a self, periods_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        periods_row
            .with_hover_card(Box::new(move |card_clock| {
                self.ccusage_periods.sections_described_by(|snapshot| {
                    claude_ccusage_periods_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::Refresh(
                ClaudeLocalSource::CcusagePeriods,
            ))
    }

    fn ccusage_block_item<'a>(&'a self, block_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        block_row
            .with_hover_card(Box::new(move |card_clock| {
                self.ccusage_block.sections_described_by(|snapshot| {
                    claude_ccusage_block_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::Refresh(
                ClaudeLocalSource::CcusageBlock,
            ))
    }

    fn usage_windows_item<'a>(&'a self, windows_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        windows_row
            .with_hover_card(Box::new(move |card_clock| {
                self.usage_windows.sections_described_by(|snapshot| {
                    claude_usage_windows_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::OnBuses(
                ClaudeUsageBusRequest::RefreshUsageWindows,
            ))
    }

    pub fn group_content(&self, clock: Clock) -> WidgetGroupContent<'_> {
        let group_summary = self.usage_windows_summary(clock);
        let mut rows = Vec::new();
        self.push_enterprise_spend_rows(&mut rows, clock);
        self.push_ccusage_periods_rows(&mut rows, clock);
        self.push_ccusage_block_rows(&mut rows, clock);
        self.push_model_ranking_rows(&mut rows);
        self.push_billed_ranking_rows(&mut rows);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::ClaudeUsage,
            group_summary,
            &self.expansion,
            ClaudeUsageInteraction::Expansion,
            rows,
        )
    }

    fn usage_windows_summary(&self, clock: Clock) -> GroupSummary<'_> {
        match &self.usage_windows {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                SimpleIcon::Claude.into(),
                self.usage_windows_item(claude_usage_windows_indicator(held_snapshot, clock)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    SimpleIcon::Claude.into(),
                    self.usage_windows_item(troubled_snapshot.shown_over(
                        claude_usage_windows_indicator(&troubled_snapshot.held_snapshot, clock),
                        clock,
                    )),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                SimpleIcon::Claude.into(),
                self.usage_windows_item(
                    placeholder.indicator(Phosphor::Gauge(Weight::Regular).into()),
                ),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                GroupSummary::glyph(SimpleIcon::Claude.into(), IndicatorState::Pending)
            }
            SubscribedSnapshot::SourceUnavailable => {
                GroupSummary::glyph(SimpleIcon::Claude.into(), IndicatorState::Off)
            }
        }
    }

    fn push_enterprise_spend_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        match &self.enterprise_spend {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let spend_row = claude_enterprise_indicator(held_snapshot, clock);
                if spend_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.enterprise_spend_item(spend_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let spend_row = troubled_snapshot.shown_over(
                    claude_enterprise_indicator(&troubled_snapshot.held_snapshot, clock),
                    clock,
                );
                if spend_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.enterprise_spend_item(spend_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.enterprise_spend_item(
                    placeholder.indicator(CLAUDE_ENTERPRISE_SPEND_FALLBACK_GLYPH),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_ccusage_periods_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        match &self.ccusage_periods {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let periods_row = claude_ccusage_periods_indicator(held_snapshot, clock);
                if periods_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.ccusage_periods_item(periods_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let periods_row = troubled_snapshot.shown_over(
                    claude_ccusage_periods_indicator(&troubled_snapshot.held_snapshot, clock),
                    clock,
                );
                if periods_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.ccusage_periods_item(periods_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.ccusage_periods_item(
                    placeholder.indicator(CLAUDE_CCUSAGE_PERIODS_FALLBACK_GLYPH),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_ccusage_block_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        match &self.ccusage_block {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let block_row = claude_ccusage_block_indicator(held_snapshot);
                if block_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.ccusage_block_item(block_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let block_row = troubled_snapshot.shown_over(
                    claude_ccusage_block_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if block_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.ccusage_block_item(block_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.ccusage_block_item(
                    placeholder.indicator(CLAUDE_CCUSAGE_BLOCK_FALLBACK_GLYPH),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_model_ranking_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(ranking) = self.model_ranking() {
            rows.push(
                ranking
                    .most_used_model_row()
                    .with_hover_card(Box::new(move |_| {
                        self.ccusage_models.hover_card_with(ranking.hover_card())
                    }))
                    .refreshed_by(ClaudeUsageInteraction::Refresh(
                        ClaudeLocalSource::CcusageModels,
                    )),
            );
        }
    }

    fn push_billed_ranking_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(billed_ranking) = self.billed_model_ranking() {
            rows.push(
                billed_ranking
                    .most_used_spend_row()
                    .with_hover_card(Box::new(move |_| {
                        self.ccusage_models
                            .hover_card_with(billed_ranking.top_model_share())
                    }))
                    .refreshed_by(ClaudeUsageInteraction::Refresh(
                        ClaudeLocalSource::CcusageModels,
                    )),
            );
        }
    }
}

fn enterprise_account(
    credentials: PathBuf,
    config: &Config,
    working_days: WeekdaySet,
    buses: &DesktopBuses,
) -> OnBus<ClaudeEnterpriseAccount> {
    OnBus {
        buses: buses.clone(),
        inputs: ClaudeEnterpriseAccount {
            credentials,
            working_time: WorkingTime::new(config, working_days),
        },
    }
}
