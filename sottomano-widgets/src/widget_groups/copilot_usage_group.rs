use crate::{
    group_items::reading_display_state::ReadingDisplayState,
    widget_groups::copilot_usage_group::{
        copilot_cli_monthly_spend::{
            COPILOT_CCUSAGE_FALLBACK_GLYPH, copilot_ccusage_hover_card,
            copilot_ccusage_panel_visibility, copilot_ccusage_reading_row,
        },
        copilot_free_quota_count::{
            COPILOT_FREE_FALLBACK_GLYPH, copilot_free_hover_card, copilot_free_reading_row,
        },
        copilot_quota_gauge::{
            COPILOT_METERED_FALLBACK_GLYPH, copilot_metered_hover_card, copilot_metered_reading_row,
        },
    },
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
    group_items::group_item::GroupItem,
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        most_used_model::{ModelSpendRanking, RankedSpan},
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
            CopilotUsageMessage::Metered(reading) => self.metered.receive(reading, clock),
            CopilotUsageMessage::Free(reading) => self.free.receive(reading, clock),
            CopilotUsageMessage::Cli(reading) => self.cli.receive(reading, clock),
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
        let cli = self.cli.visible_reading(copilot_ccusage_panel_visibility)?;
        ModelSpendRanking::new(
            "ccusage, local Copilot CLI session data",
            [RankedSpan {
                heading: "Month",
                models: &cli.period.usage.models,
            }],
        )
    }

    pub fn group_content(&self, clock: Clock) -> WidgetGroupContent<'_, CopilotUsageInteraction> {
        let refresh_account =
            |source| CopilotUsageInteraction::OnBuses(CopilotUsageBusRequest::Refresh(source));
        WidgetGroupContent::expandable(
            WidgetGroupFamily::CopilotUsage,
            &self.expansion,
            CopilotUsageInteraction::Expansion,
            [
                self.metered
                    .group_item(
                        COPILOT_METERED_FALLBACK_GLYPH,
                        |snapshot| copilot_metered_reading_row(snapshot, clock),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| {
                            self.metered
                                .sections_described_by(copilot_metered_hover_card)
                        }))
                        .previewed_on_handle()
                        .refreshed_by(refresh_account(CopilotAccountSource::Metered))
                    }),
                self.free
                    .group_item(
                        COPILOT_FREE_FALLBACK_GLYPH,
                        |snapshot| copilot_free_reading_row(snapshot),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| {
                            self.free.sections_described_by(copilot_free_hover_card)
                        }))
                        .refreshed_by(refresh_account(CopilotAccountSource::Free))
                    }),
                self.cli
                    .group_item(
                        COPILOT_CCUSAGE_FALLBACK_GLYPH,
                        |snapshot| copilot_ccusage_reading_row(snapshot, clock),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(move |_| {
                            self.cli.sections_described_by(copilot_ccusage_hover_card)
                        }))
                        .refreshed_by(CopilotUsageInteraction::RefreshCli)
                    }),
            ]
            .into_iter()
            .flatten()
            .chain(self.model_ranking().into_iter().flat_map(|ranking| {
                [
                    GroupItem::reading(ranking.most_used_model_row())
                        .with_hover_card(Box::new(move |_| {
                            self.cli.hover_card_with(ranking.hover_card())
                        }))
                        .refreshed_by(CopilotUsageInteraction::RefreshCli),
                    GroupItem::reading(ranking.most_used_spend_row())
                        .with_hover_card(Box::new(move |_| {
                            self.cli.hover_card_with(ranking.top_model_share())
                        }))
                        .refreshed_by(CopilotUsageInteraction::RefreshCli),
                ]
            })),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: SimpleIcon::GitHubCopilot.into(),
            state: ReadingDisplayState::Idle,
        })
    }
}

fn cli_budget(config: &Config, working_days: WeekdaySet) -> SpendBudget {
    SpendBudget {
        pacing: SpendPacing::WorkingTime(WorkingTime::new(config, working_days)),
        monthly_limit_usd: config.widget_groups.copilot_usage.monthly_budget_usd,
    }
}
