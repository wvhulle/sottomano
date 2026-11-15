mod best_models_within_cap;
mod openrouter_soft_budget;
mod strict_budget_remaining;

use std::{convert::identity, path::PathBuf};

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        openrouter_credits::{OpenRouterAccount, OpenRouterSpend, OpenRouterStrictBudget},
        openrouter_model_rankings::{ModelRankingInputs, OpenRouterModelRankings},
        working_time_forecast::{SpendBudget, SpendPacing},
    },
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, OnBus},
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
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
pub enum OpenRouterCreditTarget {
    SoftBudget,
    StrictBudget,
    MostUsedModel,
    MostUsedModelSpend,
    BestModelsWithinCap,
}

impl From<OpenRouterCreditTarget> for HoverTarget {
    fn from(target: OpenRouterCreditTarget) -> Self {
        HoverTarget::OpenRouterCredit(target)
    }
}

pub struct OpenRouterCreditGroup {
    spend: SubscribedSnapshot<OpenRouterSpend>,
    strict_budget: SubscribedSnapshot<OpenRouterStrictBudget>,
    model_rankings: SubscribedSnapshot<OpenRouterModelRankings>,
    management_key: Option<PathBuf>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
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
            spend: SubscribedSnapshot::default(),
            strict_budget: SubscribedSnapshot::default(),
            model_rankings: SubscribedSnapshot::default(),
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
                    first_delivery::<OpenRouterStrictBudget>(OnBus {
                        buses: buses.clone(),
                        inputs: key,
                    })
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
                snapshot_subscription::<OpenRouterStrictBudget>(OnBus {
                    buses: buses.clone(),
                    inputs: key.clone(),
                })
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
            OpenRouterCreditMessage::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
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
    pub fn hover_card_for(&self, target: OpenRouterCreditTarget, clock: Clock) -> Vec<Section> {
        match target {
            OpenRouterCreditTarget::SoftBudget => self.spend.hover_card(clock),
            OpenRouterCreditTarget::StrictBudget => self.strict_budget.hover_card(clock),
            OpenRouterCreditTarget::BestModelsWithinCap => self.model_rankings.hover_card(clock),
            OpenRouterCreditTarget::MostUsedModel => {
                let Some(ranking) = self.model_ranking() else {
                    return Vec::new();
                };
                self.spend
                    .hover_card_with(MostUsedModel(ranking).hover_card(clock))
            }
            OpenRouterCreditTarget::MostUsedModelSpend => {
                let Some(ranking) = self.model_ranking() else {
                    return Vec::new();
                };
                self.spend
                    .hover_card_with(MostUsedModelSpend(ranking).hover_card(clock))
            }
        }
    }

    pub fn group_content(&self) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::OpenRouterCredit,
            &self.expansion,
            identity,
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

fn account(
    management_key: PathBuf,
    config: &Config,
    buses: &DesktopBuses,
) -> OnBus<OpenRouterAccount> {
    let budget = &config.widget_groups.openrouter_credit;
    OnBus {
        buses: buses.clone(),
        inputs: OpenRouterAccount {
            management_key,
            budget: SpendBudget {
                pacing: SpendPacing::CalendarTime,
                monthly_limit_usd: budget.monthly_budget_usd,
            },
            daily_warning_usd: budget.daily_warning_usd,
        },
    }
}

fn ranking_inputs(
    management_key: PathBuf,
    config: &Config,
    buses: &DesktopBuses,
) -> OnBus<ModelRankingInputs> {
    OnBus {
        buses: buses.clone(),
        inputs: ModelRankingInputs {
            management_key,
            max_blended_per_million: config
                .widget_groups
                .openrouter_credit
                .max_blended_usd_per_million,
        },
    }
}
