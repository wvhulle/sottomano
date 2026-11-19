use crate::indicator_groups::copilot_usage_group::{
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
    glyph_icons::{
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
    },
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::{
        most_used_model::{ModelSpendRanking, RankedSpan},
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
    Expansion(IndicatorGroupExpansionMessage),
    RefreshCli,
    OnBuses(CopilotUsageBusRequest),
}

pub struct CopilotUsageGroup {
    metered: SubscribedSnapshot<CopilotMetered>,
    free: SubscribedSnapshot<CopilotFree>,
    cli: SubscribedSnapshot<CopilotCcusage>,
    gh_auth_token: Option<PathBuf>,
    expansion: IndicatorGroupExpansion,
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
            expansion: IndicatorGroupExpansion::default(),
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

    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.metered_headline(clock);
        let mut rows = Vec::new();
        self.push_free_indicators(&mut rows);
        self.push_cli_indicators(&mut rows, clock);
        self.push_model_ranking_indicators(&mut rows);
        self.list_metered(clock, &mut rows);
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::CopilotUsage,
            headline_indicator,
            &self.expansion,
            CopilotUsageInteraction::Expansion,
            rows,
        )
    }

    fn metered_headline(&self, clock: Clock) -> GroupIndicator<'_> {
        self.metered_indicator(self.metered.indicator(
            |held_snapshot| copilot_metered_indicator(held_snapshot, clock),
            Phosphor::Gauge(Weight::Regular).into(),
        ))
        .with_headline_glyph(SimpleIcon::GitHubCopilot.into())
    }

    fn list_metered<'a>(&'a self, clock: Clock, indicators: &mut Vec<GroupIndicator<'a>>) {
        if !self.metered.awaits_first_delivery() {
            indicators.insert(
                0,
                self.metered_indicator(self.metered.indicator(
                    |held_snapshot| copilot_metered_indicator(held_snapshot, clock),
                    Phosphor::Gauge(Weight::Regular).into(),
                )),
            );
        }
    }

    fn push_free_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if !self.free.awaits_first_delivery() {
            let free_row = self.free.indicator(
                |held_snapshot| copilot_free_indicator(held_snapshot),
                COPILOT_FREE_FALLBACK_GLYPH,
            );
            if free_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.free_indicator(free_row));
            }
        }
    }

    fn push_cli_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>, clock: Clock) {
        if !self.cli.awaits_first_delivery() {
            let cli_row = self.cli.indicator(
                |held_snapshot| copilot_ccusage_indicator(held_snapshot, clock),
                COPILOT_CCUSAGE_FALLBACK_GLYPH,
            );
            if cli_row.panel_visibility == PanelVisibility::Shown {
                rows.push(self.cli_indicator(cli_row));
            }
        }
    }

    fn push_model_ranking_indicators<'a>(&'a self, rows: &mut Vec<GroupIndicator<'a>>) {
        if let Some(ranking) = self.model_ranking() {
            rows.push(
                ranking
                    .most_used_model_indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.cli.hover_card_with(ranking.hover_card())
                    }))
                    .refreshed_by(CopilotUsageInteraction::RefreshCli),
            );
            rows.push(
                ranking
                    .most_used_spend_indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.cli.hover_card_with(ranking.top_model_share())
                    }))
                    .refreshed_by(CopilotUsageInteraction::RefreshCli),
            );
        }
    }

    fn metered_indicator<'a>(&'a self, metered_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        metered_row
            .with_hover_card(Box::new(move |_| {
                self.metered
                    .sections_described_by(copilot_metered_hover_card)
            }))
            .refreshed_by(CopilotUsageInteraction::OnBuses(
                CopilotUsageBusRequest::Refresh(CopilotAccountSource::Metered),
            ))
    }

    fn free_indicator<'a>(&'a self, free_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        free_row
            .with_hover_card(Box::new(move |_| {
                self.free.sections_described_by(copilot_free_hover_card)
            }))
            .refreshed_by(CopilotUsageInteraction::OnBuses(
                CopilotUsageBusRequest::Refresh(CopilotAccountSource::Free),
            ))
    }

    fn cli_indicator<'a>(&'a self, cli_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        cli_row
            .with_hover_card(Box::new(move |_| {
                self.cli.sections_described_by(copilot_ccusage_hover_card)
            }))
            .refreshed_by(CopilotUsageInteraction::RefreshCli)
    }
}

fn cli_budget(config: &Config, working_days: WeekdaySet) -> SpendBudget {
    SpendBudget {
        pacing: SpendPacing::WorkingTime(WorkingTime::new(config, working_days)),
        monthly_limit_usd: config.indicator_groups.copilot_usage.monthly_budget_usd,
    }
}
