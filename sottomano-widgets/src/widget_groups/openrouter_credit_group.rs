use crate::{
    group_items::reading_display_state::ReadingDisplayState,
    widget_groups::openrouter_credit_group::{
        best_models_within_cap::{
            OPEN_ROUTER_MODEL_RANKINGS_FALLBACK_GLYPH, best_models_hover_card,
            best_models_reading_row,
        },
        openrouter_soft_budget::{
            OPEN_ROUTER_SPEND_FALLBACK_GLYPH, spend_hover_card, spend_panel_visibility,
            spend_reading_row,
        },
        strict_budget_remaining::{
            OPEN_ROUTER_STRICT_BUDGET_FALLBACK_GLYPH, strict_budget_hover_card,
            strict_budget_reading_row,
        },
    },
};
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
    group_items::group_item::GroupItem,
    hover_cards::{HoverTarget, hover_card_body::Section},
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
        let spend = self.spend.visible_reading(spend_panel_visibility)?;
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
            OpenRouterCreditTarget::SoftBudget => self
                .spend
                .sections_described_by(|snapshot| spend_hover_card(snapshot, clock)),
            OpenRouterCreditTarget::StrictBudget => self
                .strict_budget
                .sections_described_by(strict_budget_hover_card),
            OpenRouterCreditTarget::BestModelsWithinCap => self
                .model_rankings
                .sections_described_by(best_models_hover_card),
            OpenRouterCreditTarget::MostUsedModel => {
                let Some(ranking) = self.model_ranking() else {
                    return Vec::new();
                };
                self.spend.hover_card_with(ranking.hover_card())
            }
            OpenRouterCreditTarget::MostUsedModelSpend => {
                let Some(ranking) = self.model_ranking() else {
                    return Vec::new();
                };
                self.spend.hover_card_with(ranking.top_model_share())
            }
        }
    }

    pub fn group_content(
        &self,
        clock: Clock,
    ) -> WidgetGroupContent<'_, WidgetGroupExpansionMessage> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::OpenRouterCredit,
            &self.expansion,
            identity,
            [
                self.spend
                    .group_item(
                        OPEN_ROUTER_SPEND_FALLBACK_GLYPH,
                        |snapshot| spend_reading_row(snapshot, clock),
                        OpenRouterCreditTarget::SoftBudget,
                        clock,
                    )
                    .map(GroupItem::previewed_on_handle),
                self.strict_budget.group_item(
                    OPEN_ROUTER_STRICT_BUDGET_FALLBACK_GLYPH,
                    |snapshot| strict_budget_reading_row(snapshot),
                    OpenRouterCreditTarget::StrictBudget,
                    clock,
                ),
                self.model_rankings.group_item(
                    OPEN_ROUTER_MODEL_RANKINGS_FALLBACK_GLYPH,
                    |snapshot| best_models_reading_row(snapshot),
                    OpenRouterCreditTarget::BestModelsWithinCap,
                    clock,
                ),
            ]
            .into_iter()
            .flatten()
            .chain(self.model_ranking().into_iter().flat_map(|ranking| {
                [
                    GroupItem::reading(
                        ranking.most_used_model_row(),
                        OpenRouterCreditTarget::MostUsedModel,
                    ),
                    GroupItem::reading(
                        ranking.most_used_spend_row(),
                        OpenRouterCreditTarget::MostUsedModelSpend,
                    ),
                ]
            })),
        )
        .face(WidgetGroupFace::Glyph {
            glyph: SimpleIcon::OpenRouter.into(),
            state: ReadingDisplayState::Idle,
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
