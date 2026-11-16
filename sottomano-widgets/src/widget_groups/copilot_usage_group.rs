use crate::widget_groups::copilot_usage_group::{
    copilot_cli_monthly_spend::{
        COPILOT_CCUSAGE_FALLBACK_GLYPH, copilot_ccusage_hover_card, copilot_ccusage_indicator,
        copilot_ccusage_panel_visibility,
    },
    copilot_free_quota_count::{
        COPILOT_FREE_FALLBACK_GLYPH, copilot_free_hover_card, copilot_free_indicator,
    },
    copilot_quota_gauge::{copilot_metered_hover_card, copilot_metered_indicator},
};
mod copilot_cli_monthly_spend;
mod copilot_free_quota_count;
mod copilot_quota_gauge;

use std::path::PathBuf;

use chrono::WeekdaySet;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        copilot_usage::{CopilotCcusage, CopilotFree, CopilotMetered},
        working_time_forecast::{SpendBudget, SpendPacing, WorkingTime},
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_state::IndicatorState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        most_used_model::{ModelSpendRanking, RankedSpan},
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopilotAccountSource {
    Metered,
    Free,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopilotUsageBusRequest {
    Refresh(CopilotAccountSource),
}

#[derive(Debug, Clone, Copy)]
pub enum CopilotUsageInteraction {
    Expansion(WidgetGroupExpansionMessage),
    RefreshCli,
    OnBuses(CopilotUsageBusRequest),
}

pub struct CopilotUsageGroup {
    metered: SubscribedSnapshot<CopilotMetered>,
    free: SubscribedSnapshot<CopilotFree>,
    cli: SubscribedSnapshot<CopilotCcusage>,
    gh_auth_token: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum CopilotUsageMessage {
    Metered(SnapshotDelivery<CopilotMetered>),
    Free(SnapshotDelivery<CopilotFree>),
    Cli(SnapshotDelivery<CopilotCcusage>),
}

impl CopilotUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(gh_auth_token: Option<PathBuf>) -> Self {
        Self {
            metered: SubscribedSnapshot::default(),
            free: SubscribedSnapshot::default(),
            cli: SubscribedSnapshot::default(),
            gh_auth_token,
            expansion: WidgetGroupExpansion::default(),
        }
    }

    fn account(&self, buses: &DesktopBuses) -> Option<OnBus<PathBuf>> {
        Some(OnBus {
            buses: buses.clone(),
            inputs: self.gh_auth_token.clone()?,
        })
    }

    pub fn on_buses(
        &self,
        request: CopilotUsageBusRequest,
        buses: &DesktopBuses,
    ) -> Task<CopilotUsageMessage> {
        let CopilotUsageBusRequest::Refresh(source) = request;
        let Some(account) = self.account(buses) else {
            return Task::none();
        };
        match source {
            CopilotAccountSource::Metered => {
                first_delivery::<CopilotMetered>(account).map(CopilotUsageMessage::Metered)
            }
            CopilotAccountSource::Free => {
                first_delivery::<CopilotFree>(account).map(CopilotUsageMessage::Free)
            }
        }
    }

    pub fn subscription(
        config: &Config,
        working_days: WeekdaySet,
    ) -> Subscription<CopilotUsageMessage> {
        snapshot_subscription::<CopilotCcusage>(cli_budget(config, working_days))
            .map(CopilotUsageMessage::Cli)
    }

    pub fn account_subscription(&self, buses: &DesktopBuses) -> Subscription<CopilotUsageMessage> {
        Subscription::batch(self.account(buses).into_iter().flat_map(|account| {
            [
                snapshot_subscription::<CopilotMetered>(account.clone())
                    .map(CopilotUsageMessage::Metered),
                snapshot_subscription::<CopilotFree>(account).map(CopilotUsageMessage::Free),
            ]
        }))
    }

    pub fn update(
        &mut self,
        message: CopilotUsageMessage,
        clock: Clock,
    ) -> Task<CopilotUsageMessage> {
        match message {
            CopilotUsageMessage::Metered(sample) => self.metered.receive(sample, clock),
            CopilotUsageMessage::Free(sample) => self.free.receive(sample, clock),
            CopilotUsageMessage::Cli(sample) => self.cli.receive(sample, clock),
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: CopilotUsageInteraction,
        config: &Config,
        working_days: WeekdaySet,
    ) -> Task<CopilotUsageMessage> {
        match interaction {
            CopilotUsageInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            CopilotUsageInteraction::RefreshCli => {
                first_delivery::<CopilotCcusage>(cli_budget(config, working_days))
                    .map(CopilotUsageMessage::Cli)
            }
            CopilotUsageInteraction::OnBuses(_) => Task::none(),
        }
    }

    fn model_ranking(&self) -> Option<ModelSpendRanking<'_, 1>> {
        let cli = self
            .cli
            .visible_indicator(copilot_ccusage_panel_visibility)?;
        ModelSpendRanking::new(
            "ccusage, local Copilot CLI session data",
            [RankedSpan {
                heading: "Month",
                models: &cli.period.usage.models,
            }],
        )
    }

    pub fn group_content(&self, clock: Clock) -> WidgetGroupContent<'_> {
        let group_summary = self.metered_summary(clock);
        let mut rows = Vec::new();
        self.push_free_rows(&mut rows, clock);
        self.push_cli_rows(&mut rows, clock);
        self.push_model_ranking_rows(&mut rows);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::CopilotUsage,
            group_summary,
            &self.expansion,
            CopilotUsageInteraction::Expansion,
            rows,
        )
    }

    fn metered_summary(&self, clock: Clock) -> GroupSummary<'_> {
        match &self.metered {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                SimpleIcon::GitHubCopilot.into(),
                self.metered_item(copilot_metered_indicator(held_snapshot, clock)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    SimpleIcon::GitHubCopilot.into(),
                    self.metered_item(troubled_snapshot.shown_over(
                        copilot_metered_indicator(&troubled_snapshot.held_snapshot, clock),
                        clock,
                    )),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                SimpleIcon::GitHubCopilot.into(),
                self.metered_item(placeholder.reading_row(Phosphor::Gauge(Weight::Regular).into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                GroupSummary::glyph(SimpleIcon::GitHubCopilot.into(), IndicatorState::Pending)
            }
            SubscribedSnapshot::SourceUnavailable => {
                GroupSummary::glyph(SimpleIcon::GitHubCopilot.into(), IndicatorState::Off)
            }
        }
    }

    fn push_free_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        match &self.free {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let free_row = copilot_free_indicator(held_snapshot);
                if free_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.free_item(free_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let free_row = troubled_snapshot.shown_over(
                    copilot_free_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if free_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.free_item(free_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.free_item(placeholder.reading_row(COPILOT_FREE_FALLBACK_GLYPH)));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_cli_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        match &self.cli {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let cli_row = copilot_ccusage_indicator(held_snapshot, clock);
                if cli_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.cli_item(cli_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let cli_row = troubled_snapshot.shown_over(
                    copilot_ccusage_indicator(&troubled_snapshot.held_snapshot, clock),
                    clock,
                );
                if cli_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.cli_item(cli_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.cli_item(placeholder.reading_row(COPILOT_CCUSAGE_FALLBACK_GLYPH)));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_model_ranking_rows<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(ranking) = self.model_ranking() {
            rows.push(
                GroupIndicator::sample(ranking.most_used_model_row())
                    .with_hover_card(Box::new(move |_| {
                        self.cli.hover_card_with(ranking.hover_card())
                    }))
                    .refreshed_by(CopilotUsageInteraction::RefreshCli),
            );
            rows.push(
                GroupIndicator::sample(ranking.most_used_spend_row())
                    .with_hover_card(Box::new(move |_| {
                        self.cli.hover_card_with(ranking.top_model_share())
                    }))
                    .refreshed_by(CopilotUsageInteraction::RefreshCli),
            );
        }
    }

    fn metered_item<'a>(&'a self, metered_row: ReadingRow<'a>) -> GroupIndicator<'a> {
        GroupIndicator::sample(metered_row)
            .with_hover_card(Box::new(move |_| {
                self.metered
                    .sections_described_by(copilot_metered_hover_card)
            }))
            .refreshed_by(CopilotUsageInteraction::OnBuses(
                CopilotUsageBusRequest::Refresh(CopilotAccountSource::Metered),
            ))
    }

    fn free_item<'a>(&'a self, free_row: ReadingRow<'a>) -> GroupIndicator<'a> {
        GroupIndicator::sample(free_row)
            .with_hover_card(Box::new(move |_| {
                self.free.sections_described_by(copilot_free_hover_card)
            }))
            .refreshed_by(CopilotUsageInteraction::OnBuses(
                CopilotUsageBusRequest::Refresh(CopilotAccountSource::Free),
            ))
    }

    fn cli_item<'a>(&'a self, cli_row: ReadingRow<'a>) -> GroupIndicator<'a> {
        GroupIndicator::sample(cli_row)
            .with_hover_card(Box::new(move |_| {
                self.cli.sections_described_by(copilot_ccusage_hover_card)
            }))
            .refreshed_by(CopilotUsageInteraction::RefreshCli)
    }
}

fn cli_budget(config: &Config, working_days: WeekdaySet) -> SpendBudget {
    SpendBudget {
        pacing: SpendPacing::WorkingTime(WorkingTime::new(config, working_days)),
        monthly_limit_usd: config.widget_groups.copilot_usage.monthly_budget_usd,
    }
}
