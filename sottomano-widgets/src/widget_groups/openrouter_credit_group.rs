use crate::widget_groups::openrouter_credit_group::{
    best_models_within_cap::{
        OPEN_ROUTER_MODEL_RANKINGS_FALLBACK_GLYPH, best_models_hover_card, best_models_indicator,
    },
    openrouter_soft_budget::{spend_hover_card, spend_indicator, spend_panel_visibility},
    strict_budget_remaining::{
        OPEN_ROUTER_STRICT_BUDGET_FALLBACK_GLYPH, strict_budget_hover_card, strict_budget_indicator,
    },
};
mod best_models_within_cap;
mod openrouter_soft_budget;
mod strict_budget_remaining;

use std::path::PathBuf;

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
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
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
pub enum OpenRouterCreditSource {
    Spend,
    StrictBudget,
    ModelRankings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenRouterCreditBusRequest {
    Refresh(OpenRouterCreditSource),
}

#[derive(Debug, Clone, Copy)]
pub enum OpenRouterCreditInteraction {
    Expansion(WidgetGroupExpansionMessage),
    OnBuses(OpenRouterCreditBusRequest),
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

    pub fn on_buses(
        &self,
        request: OpenRouterCreditBusRequest,
        config: &Config,
        buses: &DesktopBuses,
    ) -> Task<OpenRouterCreditMessage> {
        let OpenRouterCreditBusRequest::Refresh(source) = request;
        match source {
            OpenRouterCreditSource::Spend => {
                self.management_key.clone().map_or_else(Task::none, |key| {
                    first_delivery::<OpenRouterSpend>(account(key, config, buses))
                        .map(OpenRouterCreditMessage::SoftBudget)
                })
            }
            OpenRouterCreditSource::StrictBudget => {
                self.management_key.clone().map_or_else(Task::none, |key| {
                    first_delivery::<OpenRouterStrictBudget>(OnBus {
                        buses: buses.clone(),
                        inputs: key,
                    })
                    .map(OpenRouterCreditMessage::StrictBudget)
                })
            }
            OpenRouterCreditSource::ModelRankings => {
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
            OpenRouterCreditMessage::SoftBudget(sample) => {
                self.spend.receive(sample, clock);
            }
            OpenRouterCreditMessage::StrictBudget(sample) => {
                self.strict_budget.receive(sample, clock);
            }
            OpenRouterCreditMessage::ModelRankings(sample) => {
                self.model_rankings.receive(sample, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: OpenRouterCreditInteraction,
    ) -> Task<OpenRouterCreditMessage> {
        match interaction {
            OpenRouterCreditInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
            }
            OpenRouterCreditInteraction::OnBuses(_) => {}
        }
        Task::none()
    }

    fn model_ranking(&self) -> Option<ModelSpendRanking<'_, 3>> {
        let spend = self.spend.visible_indicator(spend_panel_visibility)?;
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

    pub fn group_content(&self, clock: Clock) -> WidgetGroupContent<'_> {
        let group_summary = match &self.spend {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                SimpleIcon::OpenRouter.into(),
                self.spend_item(spend_indicator(held_snapshot, clock)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    SimpleIcon::OpenRouter.into(),
                    self.spend_item(troubled_snapshot.shown_over(
                        spend_indicator(&troubled_snapshot.held_snapshot, clock),
                        clock,
                    )),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                SimpleIcon::OpenRouter.into(),
                self.spend_item(placeholder.indicator(Phosphor::Coins(Weight::Regular).into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                GroupSummary::glyph(SimpleIcon::OpenRouter.into(), IndicatorState::Pending)
            }
            SubscribedSnapshot::SourceUnavailable => {
                GroupSummary::glyph(SimpleIcon::OpenRouter.into(), IndicatorState::Off)
            }
        };
        let mut rows = Vec::new();
        match &self.strict_budget {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let budget_row = strict_budget_indicator(held_snapshot);
                if budget_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.strict_budget_item(budget_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let budget_row = troubled_snapshot.shown_over(
                    strict_budget_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if budget_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.strict_budget_item(budget_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.strict_budget_item(
                    placeholder.indicator(OPEN_ROUTER_STRICT_BUDGET_FALLBACK_GLYPH),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        match &self.model_rankings {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let rankings_row = best_models_indicator(held_snapshot);
                if rankings_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.model_rankings_item(rankings_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let rankings_row = troubled_snapshot.shown_over(
                    best_models_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if rankings_row.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.model_rankings_item(rankings_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.model_rankings_item(
                    placeholder.indicator(OPEN_ROUTER_MODEL_RANKINGS_FALLBACK_GLYPH),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        if let Some(ranking) = self.model_ranking() {
            rows.push(
                ranking
                    .most_used_model_row()
                    .with_hover_card(Box::new(move |_| {
                        self.spend.hover_card_with(ranking.hover_card())
                    }))
                    .refreshed_by(credit_refresh_interaction(OpenRouterCreditSource::Spend)),
            );
            rows.push(
                ranking
                    .most_used_spend_row()
                    .with_hover_card(Box::new(move |_| {
                        self.spend.hover_card_with(ranking.top_model_share())
                    }))
                    .refreshed_by(credit_refresh_interaction(OpenRouterCreditSource::Spend)),
            );
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::OpenRouterCredit,
            group_summary,
            &self.expansion,
            OpenRouterCreditInteraction::Expansion,
            rows,
        )
    }

    fn spend_item<'a>(&'a self, spend_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        spend_row
            .with_hover_card(Box::new(move |card_clock| {
                self.spend
                    .sections_described_by(|snapshot| spend_hover_card(snapshot, card_clock))
            }))
            .refreshed_by(credit_refresh_interaction(OpenRouterCreditSource::Spend))
    }

    fn strict_budget_item<'a>(&'a self, budget_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        budget_row
            .with_hover_card(Box::new(move |_| {
                self.strict_budget
                    .sections_described_by(strict_budget_hover_card)
            }))
            .refreshed_by(credit_refresh_interaction(
                OpenRouterCreditSource::StrictBudget,
            ))
    }

    fn model_rankings_item<'a>(&'a self, rankings_row: GroupIndicator<'a>) -> GroupIndicator<'a> {
        rankings_row
            .with_hover_card(Box::new(move |_| {
                self.model_rankings
                    .sections_described_by(best_models_hover_card)
            }))
            .refreshed_by(credit_refresh_interaction(
                OpenRouterCreditSource::ModelRankings,
            ))
    }
}

fn credit_refresh_interaction(source: OpenRouterCreditSource) -> OpenRouterCreditInteraction {
    OpenRouterCreditInteraction::OnBuses(OpenRouterCreditBusRequest::Refresh(source))
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
