use chrono::Locale;

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::{ReadingRow, reading_hover_card},
    },
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

use std::{convert::identity, path::PathBuf};

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
    hover_cards::{HoverTarget, hover_card_body::Section},
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        most_used_model::{ModelSpendRanking, MostUsedModel, MostUsedModelSpend, RankedSpan},
        simple_icons::SimpleIcon,
        subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopilotUsageTarget {
    Metered,
    Free,
    Cli,
    MostUsedModel,
    MostUsedModelSpend,
}

impl From<CopilotUsageTarget> for HoverTarget {
    fn from(target: CopilotUsageTarget) -> Self {
        HoverTarget::CopilotUsage(target)
    }
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
    Expansion(WidgetGroupExpansionMessage),
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

    pub fn refresh(
        &self,
        target: CopilotUsageTarget,
        config: &Config,
        working_days: WeekdaySet,
        buses: Option<&DesktopBuses>,
    ) -> Task<CopilotUsageMessage> {
        let account = buses.and_then(|buses| self.account(buses));
        match target {
            CopilotUsageTarget::Metered => account.map_or_else(Task::none, |key| {
                first_delivery::<CopilotMetered>(key).map(CopilotUsageMessage::Metered)
            }),
            CopilotUsageTarget::Free => account.map_or_else(Task::none, |key| {
                first_delivery::<CopilotFree>(key).map(CopilotUsageMessage::Free)
            }),
            CopilotUsageTarget::Cli
            | CopilotUsageTarget::MostUsedModel
            | CopilotUsageTarget::MostUsedModelSpend => {
                first_delivery::<CopilotCcusage>(cli_budget(config, working_days))
                    .map(CopilotUsageMessage::Cli)
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
            CopilotUsageMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: CopilotUsageTarget, clock: Clock) -> Vec<Section> {
        match target {
            CopilotUsageTarget::Metered => self
                .metered
                .sections_described_by(|snapshot| copilot_metered_hover_card(snapshot, clock)),
            CopilotUsageTarget::Free => self
                .free
                .sections_described_by(|snapshot| copilot_free_hover_card(snapshot, clock)),
            CopilotUsageTarget::Cli => self
                .cli
                .sections_described_by(|snapshot| copilot_ccusage_hover_card(snapshot, clock)),
            CopilotUsageTarget::MostUsedModel => {
                let Some(ranking) = self.model_ranking() else {
                    return Vec::new();
                };
                self.cli
                    .hover_card_with(reading_hover_card(MostUsedModel(ranking), clock))
            }
            CopilotUsageTarget::MostUsedModelSpend => {
                let Some(ranking) = self.model_ranking() else {
                    return Vec::new();
                };
                self.cli
                    .hover_card_with(reading_hover_card(MostUsedModelSpend(ranking), clock))
            }
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

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::CopilotUsage,
            &self.expansion,
            identity,
            [
                self.metered
                    .group_item(
                        COPILOT_METERED_FALLBACK_GLYPH,
                        |snapshot| copilot_metered_reading_row(snapshot, clock, locale),
                        CopilotUsageTarget::Metered,
                        clock,
                    )
                    .map(GroupItem::previewed_on_handle),
                self.free.group_item(
                    COPILOT_FREE_FALLBACK_GLYPH,
                    |snapshot| copilot_free_reading_row(snapshot, clock, locale),
                    CopilotUsageTarget::Free,
                    clock,
                ),
                self.cli.group_item(
                    COPILOT_CCUSAGE_FALLBACK_GLYPH,
                    |snapshot| copilot_ccusage_reading_row(snapshot, clock, locale),
                    CopilotUsageTarget::Cli,
                    clock,
                ),
            ]
            .into_iter()
            .flatten()
            .chain(self.model_ranking().into_iter().flat_map(|ranking| {
                [
                    GroupItem::reading(
                        ReadingRow::evaluated(MostUsedModel(ranking), clock, locale),
                        CopilotUsageTarget::MostUsedModel,
                    ),
                    GroupItem::reading(
                        ReadingRow::evaluated(MostUsedModelSpend(ranking), clock, locale),
                        CopilotUsageTarget::MostUsedModelSpend,
                    ),
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
