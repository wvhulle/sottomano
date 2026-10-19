mod copilot_cli_monthly_spend;
mod copilot_free_quota_count;
mod copilot_quota_gauge;

use std::path::PathBuf;

use chrono::WeekdaySet;
use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    ai_usage_spend::{
        copilot_usage::{self, CopilotAccount, CopilotCcusage, CopilotFree, CopilotMetered},
        working_time_forecast::{SpendBudget, SpendPacing, WorkingTime},
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
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
        simple_icons::SimpleIcon,
        tooltip_body::{Section, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace},
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

impl From<copilot_usage::Error> for Section {
    fn from(error: copilot_usage::Error) -> Self {
        match error {
            copilot_usage::Error::Fetch(fetch) => fetch.into(),
            other => Section::error(other),
        }
    }
}

impl From<CopilotUsageTarget> for HoverTarget {
    fn from(target: CopilotUsageTarget) -> Self {
        HoverTarget::CopilotUsage(target)
    }
}

pub struct CopilotUsageGroup {
    metered: ContinuousReading<CopilotMetered>,
    free: ContinuousReading<CopilotFree>,
    cli: ContinuousReading<CopilotCcusage>,
    gh_auth_token: Option<PathBuf>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum CopilotUsageMessage {
    Metered(SourceDelivery<CopilotMetered>),
    Free(SourceDelivery<CopilotFree>),
    Cli(SourceDelivery<CopilotCcusage>),
    Expansion(ExpansionMessage),
}

impl CopilotUsageGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(gh_auth_token: Option<PathBuf>) -> Self {
        Self {
            metered: ContinuousReading::default(),
            free: ContinuousReading::default(),
            cli: ContinuousReading::default(),
            gh_auth_token,
            expansion: GroupExpansion::default(),
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
        source_subscription::<CopilotCcusage>(cli_budget(config, working_days))
            .map(CopilotUsageMessage::Cli)
    }

    pub fn account_subscription(&self, buses: &DesktopBuses) -> Subscription<CopilotUsageMessage> {
        Subscription::batch(self.account(buses).into_iter().flat_map(|account| {
            [
                source_subscription::<CopilotMetered>(account.clone())
                    .map(CopilotUsageMessage::Metered),
                source_subscription::<CopilotFree>(account).map(CopilotUsageMessage::Free),
            ]
        }))
    }

    pub fn update(
        &mut self,
        message: CopilotUsageMessage,
        clock: Clock,
    ) -> Task<CopilotUsageMessage> {
        match message {
            CopilotUsageMessage::Metered(reading) => self.metered.receive_delivery(reading, clock),
            CopilotUsageMessage::Free(reading) => self.free.receive_delivery(reading, clock),
            CopilotUsageMessage::Cli(reading) => self.cli.receive_delivery(reading, clock),
            CopilotUsageMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    #[must_use]
    pub fn tooltip_for(&self, target: CopilotUsageTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            CopilotUsageTarget::Metered => self.metered.tooltip(clock),
            CopilotUsageTarget::Free => self.free.tooltip(clock),
            CopilotUsageTarget::Cli => self.cli.tooltip(clock),
            CopilotUsageTarget::MostUsedModel => self
                .cli
                .tooltip_with(MostUsedModel(self.model_ranking()?).tooltip(clock)),
            CopilotUsageTarget::MostUsedModelSpend => self
                .cli
                .tooltip_with(MostUsedModelSpend(self.model_ranking()?).tooltip(clock)),
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

    pub fn group_content(&self) -> GroupContent<'_, CopilotUsageMessage> {
        GroupContent::new(
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
        .face(HandleFace::Icon {
            glyph: SimpleIcon::GithubCopilot.into(),
            handle: GroupHandle::CopilotUsage,
        })
    }
}

fn cli_budget(config: &Config, working_days: WeekdaySet) -> SpendBudget {
    SpendBudget {
        pacing: SpendPacing::WorkingTime(WorkingTime::new(config, working_days)),
        monthly_limit_usd: config.widget_groups.copilot_usage.monthly_budget_usd,
    }
}
