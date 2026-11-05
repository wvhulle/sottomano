mod best_models_within_cap;
mod openrouter_soft_budget;
mod strict_budget_remaining;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        openrouter_credits::{self, OpenRouterAccount, OpenRouterSpend, OpenRouterStrictBudget},
        openrouter_model_rankings::{ModelRankingInputs, OpenRouterModelRankings},
        working_time_forecast::{SpendBudget, SpendPacing},
    },
    network_connectivity::internet_connectivity_gate::OnlineSourceInputs,
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
pub enum OpenRouterCreditTarget {
    SoftBudget,
    StrictBudget,
    MostUsedModel,
    MostUsedModelSpend,
    BestModelsWithinCap,
}

impl From<openrouter_credits::Error> for Section {
    fn from(error: openrouter_credits::Error) -> Self {
        match error {
            openrouter_credits::Error::Fetch(fetch) => fetch.into(),
            secret @ openrouter_credits::Error::Secret => Section::error(secret),
        }
    }
}

impl From<OpenRouterCreditTarget> for HoverTarget {
    fn from(target: OpenRouterCreditTarget) -> Self {
        HoverTarget::OpenRouterCredit(target)
    }
}

pub struct OpenRouterCreditGroup {
    spend: LatestSnapshot<OpenRouterSpend>,
    strict_budget: LatestSnapshot<OpenRouterStrictBudget>,
    model_rankings: LatestSnapshot<OpenRouterModelRankings>,
    management_key: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum OpenRouterCreditMessage {
    SoftBudget(SnapshotDelivery<OpenRouterSpend>),
    StrictBudget(SnapshotDelivery<OpenRouterStrictBudget>),
    ModelRankings(SnapshotDelivery<OpenRouterModelRankings>),
    Expansion(WidgetGroupExpansionMessage),
}

impl OpenRouterCreditGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(management_key: Option<PathBuf>) -> Self {
        Self {
            spend: LatestSnapshot::default(),
            strict_budget: LatestSnapshot::default(),
            model_rankings: LatestSnapshot::default(),
            management_key,
            expansion: WidgetGroupExpansion::default(),
        }
    }

    pub fn refresh(
        &self,
        target: OpenRouterCreditTarget,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Task<OpenRouterCreditMessage> {
        let Some(buses) = buses else {
            return Task::none();
        };
        match target {
            OpenRouterCreditTarget::SoftBudget
            | OpenRouterCreditTarget::MostUsedModel
            | OpenRouterCreditTarget::MostUsedModelSpend => {
                self.management_key.clone().map_or_else(Task::none, |key| {
                    first_delivery::<OpenRouterSpend>(account(key, config, buses))
                        .map(OpenRouterCreditMessage::SoftBudget)
                })
            }
            OpenRouterCreditTarget::StrictBudget => {
                self.management_key.clone().map_or_else(Task::none, |key| {
                    first_delivery::<OpenRouterStrictBudget>(online(key, buses))
                        .map(OpenRouterCreditMessage::StrictBudget)
                })
            }
            OpenRouterCreditTarget::BestModelsWithinCap => {
                self.management_key.clone().map_or_else(Task::none, |key| {
                    first_delivery::<OpenRouterModelRankings>(ranking_inputs(key, config, buses))
                        .map(OpenRouterCreditMessage::ModelRankings)
                })
            }
        }
    }

    pub fn subscription(
        &self,
        config: &Config,
        buses: Option<&DesktopBuses>,
    ) -> Subscription<OpenRouterCreditMessage> {
        let Some(buses) = buses else {
            return Subscription::none();
        };
        Subscription::batch(self.management_key.clone().into_iter().flat_map(|key| {
            [
                snapshot_subscription::<OpenRouterSpend>(account(key.clone(), config, buses))
                    .map(OpenRouterCreditMessage::SoftBudget),
                snapshot_subscription::<OpenRouterStrictBudget>(online(key.clone(), buses))
                    .map(OpenRouterCreditMessage::StrictBudget),
                snapshot_subscription::<OpenRouterModelRankings>(ranking_inputs(
                    key, config, buses,
                ))
                .map(OpenRouterCreditMessage::ModelRankings),
            ]
        }))
    }

    pub fn update(
        &mut self,
        message: OpenRouterCreditMessage,
        clock: Clock,
    ) -> Task<OpenRouterCreditMessage> {
        match message {
            OpenRouterCreditMessage::SoftBudget(reading) => {
                self.spend.receive(reading, clock);
            }
            OpenRouterCreditMessage::StrictBudget(reading) => {
                self.strict_budget.receive(reading, clock);
            }
            OpenRouterCreditMessage::ModelRankings(reading) => {
                self.model_rankings.receive(reading, clock);
            }
            OpenRouterCreditMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    fn model_ranking(&self) -> Option<ModelSpendRanking<'_, 3>> {
        let spend = self.spend.visible_reading()?;
        ModelSpendRanking::new(
            "OpenRouter analytics, per local calendar span",
            [
                RankedSpan {
                    heading: "Today",
                    models: &spend.today.models,
                },
                RankedSpan {
                    heading: "Week",
                    models: &spend.week.models,
                },
                RankedSpan {
                    heading: "Month",
                    models: &spend.month.models,
                },
            ],
        )
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: OpenRouterCreditTarget,
        clock: Clock,
    ) -> Option<HoverCard> {
        match target {
            OpenRouterCreditTarget::SoftBudget => self.spend.hover_card(clock),
            OpenRouterCreditTarget::StrictBudget => self.strict_budget.hover_card(clock),
            OpenRouterCreditTarget::BestModelsWithinCap => self.model_rankings.hover_card(clock),
            OpenRouterCreditTarget::MostUsedModel => self
                .spend
                .hover_card_with(MostUsedModel(self.model_ranking()?).hover_card(clock)),
            OpenRouterCreditTarget::MostUsedModelSpend => self
                .spend
                .hover_card_with(MostUsedModelSpend(self.model_ranking()?).hover_card(clock)),
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, OpenRouterCreditMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::OpenRouterCredit,
            &self.expansion,
            OpenRouterCreditMessage::Expansion,
            [
                self.spend
                    .group_item(OpenRouterCreditTarget::SoftBudget)
                    .map(GroupItem::previewed_on_handle),
                self.strict_budget
                    .group_item(OpenRouterCreditTarget::StrictBudget),
                self.model_rankings
                    .group_item(OpenRouterCreditTarget::BestModelsWithinCap),
            ]
            .into_iter()
            .flatten()
            .chain(self.model_ranking().into_iter().flat_map(|ranking| {
                [
                    GroupItem::reading(
                        MostUsedModel(ranking),
                        OpenRouterCreditTarget::MostUsedModel,
                    ),
                    GroupItem::reading(
                        MostUsedModelSpend(ranking),
                        OpenRouterCreditTarget::MostUsedModelSpend,
                    ),
                ]
            })),
        )
        .face(WidgetGroupFace::Icon {
            glyph: SimpleIcon::OpenRouter.into(),
        })
    }
}

fn online<SourceInputs>(
    source_inputs: SourceInputs,
    buses: &DesktopBuses,
) -> OnlineSourceInputs<SourceInputs> {
    OnlineSourceInputs {
        buses: buses.clone(),
        source_inputs,
    }
}

fn account(
    management_key: PathBuf,
    config: &Config,
    buses: &DesktopBuses,
) -> OnlineSourceInputs<OpenRouterAccount> {
    let budget = &config.widget_groups.openrouter_credit;
    online(
        OpenRouterAccount {
            management_key,
            budget: SpendBudget {
                pacing: SpendPacing::CalendarTime,
                monthly_limit_usd: budget.monthly_budget_usd,
            },
            daily_warning_usd: budget.daily_warning_usd,
        },
        buses,
    )
}

fn ranking_inputs(
    management_key: PathBuf,
    config: &Config,
    buses: &DesktopBuses,
) -> OnlineSourceInputs<ModelRankingInputs> {
    online(
        ModelRankingInputs {
            management_key,
            max_blended_per_million: config
                .widget_groups
                .openrouter_credit
                .max_blended_usd_per_million,
        },
        buses,
    )
}
