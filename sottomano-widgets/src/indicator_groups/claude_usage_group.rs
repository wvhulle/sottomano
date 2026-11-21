use crate::indicator_groups::claude_usage_group::{
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
use sottomano_toml_config::{Config, secrets::StoredSecret};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    indicator_groups::{
        claude_usage_group::claude_enterprise_credit_spend::claude_organization_sections,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        most_used_model::{ModelSpendRanking, RankedSpan},
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
    Expansion(IndicatorGroupExpansionMessage),
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
    oauth_credentials: StoredSecret,
    organization: HoverFetchedSnapshot<ClaudeOrganization>,
    expansion: IndicatorGroupExpansion,
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
    pub fn new(oauth_credentials: StoredSecret) -> Self {
        Self {
            usage_windows: SubscribedSnapshot::default(),
            enterprise_spend: SubscribedSnapshot::default(),
            ccusage_periods: SubscribedSnapshot::default(),
            ccusage_block: SubscribedSnapshot::default(),
            ccusage_models: SubscribedSnapshot::default(),
            oauth_credentials,
            organization: HoverFetchedSnapshot::default(),
            expansion: IndicatorGroupExpansion::default(),
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
        let StoredSecret::Present(credentials) = self.oauth_credentials.clone() else {
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
                monthly_limit_usd: config.indicator_groups.claude_usage.monthly_budget_usd,
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
        let local_logs = self.local_logs(config);
        let mut subscriptions = vec![
            snapshot_subscription::<ClaudeCcusageBlock>(local_logs.clone())
                .map(ClaudeUsageMessage::CcusageBlock),
            snapshot_subscription::<ClaudeCcusagePeriods>(local_logs)
                .map(ClaudeUsageMessage::CcusagePeriods),
            snapshot_subscription::<ClaudeCcusageModels>(self.oauth_credentials.clone())
                .map(ClaudeUsageMessage::CcusageModels),
        ];
        if let StoredSecret::Present(key) = &self.oauth_credentials
            && let Some(buses) = buses
        {
            subscriptions.extend([
                snapshot_subscription::<ClaudeUsageWindows>(OnBus {
                    buses: buses.clone(),
                    inputs: key.clone(),
                })
                .map(|sample| ClaudeUsageMessage::UsageWindows(Box::new(sample))),
                snapshot_subscription::<ClaudeEnterpriseSpend>(enterprise_account(
                    key.clone(),
                    config,
                    working_days,
                    buses,
                ))
                .map(ClaudeUsageMessage::EnterpriseSpend),
            ]);
        }
        Subscription::batch(subscriptions)
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

    fn enterprise_spend_indicator<'a>(
        &'a self,
        spend_row: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
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

    fn ccusage_periods_indicator<'a>(
        &'a self,
        periods_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        periods_indicator
            .with_hover_card(Box::new(move |card_clock| {
                self.ccusage_periods.sections_described_by(|snapshot| {
                    claude_ccusage_periods_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::Refresh(
                ClaudeLocalSource::CcusagePeriods,
            ))
    }

    fn ccusage_block_indicator<'a>(
        &'a self,
        block_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        block_indicator
            .with_hover_card(Box::new(move |card_clock| {
                self.ccusage_block.sections_described_by(|snapshot| {
                    claude_ccusage_block_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::Refresh(
                ClaudeLocalSource::CcusageBlock,
            ))
    }

    fn usage_windows_indicator<'a>(
        &'a self,
        windows_row: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
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

    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.usage_windows_headline(clock);
        let mut rows = Vec::new();
        self.push_enterprise_spend_indicators(&mut rows, clock);
        self.push_ccusage_periods_indicators(&mut rows, clock);
        self.push_ccusage_block_indicators(&mut rows);
        self.push_model_ranking_indicators(&mut rows);
        self.push_billed_ranking_indicators(&mut rows);
        self.list_usage_windows(clock, &mut rows);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::ClaudeUsage,
            headline_indicator,
            &self.expansion,
            ClaudeUsageInteraction::Expansion,
            rows,
        )
    }

    fn usage_windows_headline(&self, clock: Clock) -> GroupIndicator<'_> {
        self.usage_windows_indicator(self.usage_windows.indicator(
            |held_snapshot| claude_usage_windows_indicator(held_snapshot, clock),
            Phosphor::Gauge(Weight::Regular).into(),
        ))
        .with_headline_glyph(SimpleIcon::Claude.into())
    }

    fn list_usage_windows<'a>(&'a self, clock: Clock, indicators: &mut Vec<GroupIndicator<'a>>) {
        if !self.usage_windows.awaits_first_delivery() {
            indicators.insert(
                0,
                self.usage_windows_indicator(self.usage_windows.indicator(
                    |held_snapshot| claude_usage_windows_indicator(held_snapshot, clock),
                    Phosphor::Gauge(Weight::Regular).into(),
                )),
            );
        }
    }

    fn push_enterprise_spend_indicators<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
    ) {
        if !self.enterprise_spend.awaits_first_delivery() {
            let spend_row = self.enterprise_spend.indicator(
                |held_snapshot| claude_enterprise_indicator(held_snapshot, clock),
                CLAUDE_ENTERPRISE_SPEND_FALLBACK_GLYPH,
            );
            if spend_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.enterprise_spend_indicator(spend_row));
            }
        }
    }

    fn push_ccusage_periods_indicators<'a>(
        &'a self,
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
    ) {
        if !self.ccusage_periods.awaits_first_delivery() {
            let periods_indicator = self.ccusage_periods.indicator(
                |held_snapshot| claude_ccusage_periods_indicator(held_snapshot, clock),
                CLAUDE_CCUSAGE_PERIODS_FALLBACK_GLYPH,
            );
            if periods_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.ccusage_periods_indicator(periods_indicator));
            }
        }
    }

    fn push_ccusage_block_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if !self.ccusage_block.awaits_first_delivery() {
            let block_indicator = self.ccusage_block.indicator(
                |held_snapshot| claude_ccusage_block_indicator(held_snapshot),
                CLAUDE_CCUSAGE_BLOCK_FALLBACK_GLYPH,
            );
            if block_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.ccusage_block_indicator(block_indicator));
            }
        }
    }

    fn push_model_ranking_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(ranking) = self.model_ranking() {
            rows.push(
                ranking
                    .most_used_model_indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.ccusage_models.hover_card_with(ranking.hover_card())
                    }))
                    .refreshed_by(ClaudeUsageInteraction::Refresh(
                        ClaudeLocalSource::CcusageModels,
                    )),
            );
        }
    }

    fn push_billed_ranking_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(billed_ranking) = self.billed_model_ranking() {
            rows.push(
                billed_ranking
                    .most_used_spend_indicator()
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
