mod copilot_cli_monthly_spend;
mod copilot_free_quota_count;
mod copilot_quota_gauge;

use std::path::PathBuf;

use chrono::WeekdaySet;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        copilot_usage::{self, CopilotAccount, CopilotCcusage, CopilotFree, CopilotMetered},
        working_time_forecast::{SpendBudget, SpendPacing, WorkingTime},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage, WidgetGroupFace,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        latest_snapshot::LatestSnapshot,
        most_used_model::{ModelSpendRanking, MostUsedModel, MostUsedModelSpend, RankedSpan},
        simple_icons::SimpleIcon,
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
    metered: LatestSnapshot<CopilotMetered>,
    free: LatestSnapshot<CopilotFree>,
    cli: LatestSnapshot<CopilotCcusage>,
    gh_auth_token: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
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
            metered: LatestSnapshot::default(),
            free: LatestSnapshot::default(),
            cli: LatestSnapshot::default(),
            gh_auth_token,
            expansion: WidgetGroupExpansion::default(),
        }
    }

    fn account(&self, buses: &DesktopBuses) -> Option<CopilotAccount> {
        Some(CopilotAccount {
            gh_auth_token: self.gh_auth_token.clone()?,
            buses: buses.clone(),
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
            CopilotUsageMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn hover_card_for(&self, target: CopilotUsageTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            CopilotUsageTarget::Metered => self.metered.hover_card(clock),
            CopilotUsageTarget::Free => self.free.hover_card(clock),
            CopilotUsageTarget::Cli => self.cli.hover_card(clock),
            CopilotUsageTarget::MostUsedModel => self
                .cli
                .hover_card_with(MostUsedModel(self.model_ranking()?).hover_card(clock)),
            CopilotUsageTarget::MostUsedModelSpend => self
                .cli
                .hover_card_with(MostUsedModelSpend(self.model_ranking()?).hover_card(clock)),
        }
    }

    fn model_ranking(&self) -> Option<ModelSpendRanking<'_, 1>> {
        let cli = self.cli.visible_reading()?;
        ModelSpendRanking::new(
            "ccusage, local Copilot CLI session data",
            [RankedSpan {
                heading: "Month",
                models: &cli.period.usage.models,
            }],
        )
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, CopilotUsageMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::CopilotUsage,
            &self.expansion,
            CopilotUsageMessage::Expansion,
            [
                self.metered
                    .group_item(CopilotUsageTarget::Metered)
                    .map(GroupItem::previewed_on_handle),
                self.free.group_item(CopilotUsageTarget::Free),
                self.cli.group_item(CopilotUsageTarget::Cli),
            ]
            .into_iter()
            .flatten()
            .chain(self.model_ranking().into_iter().flat_map(|ranking| {
                [
                    GroupItem::reading(MostUsedModel(ranking), CopilotUsageTarget::MostUsedModel),
                    GroupItem::reading(
                        MostUsedModelSpend(ranking),
                        CopilotUsageTarget::MostUsedModelSpend,
                    ),
                ]
            })),
        )
        .face(WidgetGroupFace::Icon {
            glyph: SimpleIcon::GitHubCopilot.into(),
        })
    }
}

fn cli_budget(config: &Config, working_days: WeekdaySet) -> SpendBudget {
    SpendBudget {
        pacing: SpendPacing::WorkingTime(WorkingTime::new(config, working_days)),
        monthly_limit_usd: config.widget_groups.copilot_usage.monthly_budget_usd,
    }
}
