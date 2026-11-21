use crate::indicator_groups::openrouter_credit_group::{
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
use sottomano_toml_config::{Config, secrets::StoredSecret};

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
    Expansion(IndicatorGroupExpansionMessage),
    OnBuses(OpenRouterCreditBusRequest),
}

pub struct OpenRouterCreditGroup {
    spend: SubscribedSnapshot<OpenRouterSpend>,
    strict_budget: SubscribedSnapshot<OpenRouterStrictBudget>,
    model_rankings: SubscribedSnapshot<OpenRouterModelRankings>,
    management_key: StoredSecret,
    expansion: IndicatorGroupExpansion,
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
    pub fn new(management_key: StoredSecret) -> Self {
        Self {
            spend: SubscribedSnapshot::default(),
            strict_budget: SubscribedSnapshot::default(),
            model_rankings: SubscribedSnapshot::default(),
            management_key,
            expansion: IndicatorGroupExpansion::default(),
        }
    }

    pub fn on_buses(
        &self,
        request: OpenRouterCreditBusRequest,
        config: &Config,
        buses: &DesktopBuses,
    ) -> Task<OpenRouterCreditMessage> {
        let OpenRouterCreditBusRequest::Refresh(source) = request;
        let StoredSecret::Present(key) = self.management_key.clone() else {
            return Task::none();
        };
        match source {
            OpenRouterCreditSource::Spend => {
                first_delivery::<OpenRouterSpend>(account(key, config, buses))
                    .map(OpenRouterCreditMessage::SoftBudget)
            }
            OpenRouterCreditSource::StrictBudget => {
                first_delivery::<OpenRouterStrictBudget>(OnBus {
                    buses: buses.clone(),
                    inputs: key,
                })
                .map(OpenRouterCreditMessage::StrictBudget)
            }
            OpenRouterCreditSource::ModelRankings => {
                first_delivery::<OpenRouterModelRankings>(ranking_inputs(key, config, buses))
                    .map(OpenRouterCreditMessage::ModelRankings)
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
        let StoredSecret::Present(key) = self.management_key.clone() else {
            return Subscription::none();
        };
        Subscription::batch([
            snapshot_subscription::<OpenRouterSpend>(account(key.clone(), config, buses))
                .map(OpenRouterCreditMessage::SoftBudget),
            snapshot_subscription::<OpenRouterStrictBudget>(OnBus {
                buses: buses.clone(),
                inputs: key.clone(),
            })
            .map(OpenRouterCreditMessage::StrictBudget),
            snapshot_subscription::<OpenRouterModelRankings>(ranking_inputs(key, config, buses))
                .map(OpenRouterCreditMessage::ModelRankings),
        ])
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

    fn list_spend<'view>(
        &'view self,
        clock: Clock,
        lead_indicators: &mut Vec<GroupIndicator<'view>>,
    ) {
        if !self.spend.awaits_first_delivery() {
            lead_indicators.push(self.wired_spend_indicator(self.spend.indicator(
                |held_snapshot| spend_indicator(held_snapshot, clock),
                Phosphor::Coins(Weight::Regular).into(),
            )));
        }
    }

    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let headline_indicator = self
            .wired_spend_indicator(self.spend.indicator(
                |held_snapshot| spend_indicator(held_snapshot, clock),
                Phosphor::Coins(Weight::Regular).into(),
            ))
            .with_headline_glyph(SimpleIcon::OpenRouter.into());
        let mut lead_indicators = Vec::new();
        self.list_spend(clock, &mut lead_indicators);
        let mut rows = Vec::new();
        if !self.strict_budget.awaits_first_delivery() {
            let budget_indicator = self.strict_budget.indicator(
                |held_snapshot| strict_budget_indicator(held_snapshot),
                OPEN_ROUTER_STRICT_BUDGET_FALLBACK_GLYPH,
            );
            if budget_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.wired_strict_budget_indicator(budget_indicator));
            }
        }
        if !self.model_rankings.awaits_first_delivery() {
            let rankings_indicator = self.model_rankings.indicator(
                |held_snapshot| best_models_indicator(held_snapshot),
                OPEN_ROUTER_MODEL_RANKINGS_FALLBACK_GLYPH,
            );
            if rankings_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.model_rankings_indicator(rankings_indicator));
            }
        }
        if let Some(ranking) = self.model_ranking() {
            rows.push(
                ranking
                    .most_used_model_indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.spend.hover_card_with(ranking.hover_card())
                    }))
                    .refreshed_by(credit_refresh_interaction(OpenRouterCreditSource::Spend)),
            );
            rows.push(
                ranking
                    .most_used_spend_indicator()
                    .with_hover_card(Box::new(move |_| {
                        self.spend.hover_card_with(ranking.top_model_share())
                    }))
                    .refreshed_by(credit_refresh_interaction(OpenRouterCreditSource::Spend)),
            );
        }
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::OpenRouterCredit,
            headline_indicator,
            &self.expansion,
            OpenRouterCreditInteraction::Expansion,
            lead_indicators.into_iter().chain(rows),
        )
    }

    fn wired_spend_indicator<'view>(
        &'view self,
        spend_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        spend_row
            .with_hover_card(Box::new(move |card_clock| {
                self.spend
                    .sections_described_by(|snapshot| spend_hover_card(snapshot, card_clock))
            }))
            .refreshed_by(credit_refresh_interaction(OpenRouterCreditSource::Spend))
    }

    fn wired_strict_budget_indicator<'view>(
        &'view self,
        budget_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        budget_indicator
            .with_hover_card(Box::new(move |_| {
                self.strict_budget
                    .sections_described_by(strict_budget_hover_card)
            }))
            .refreshed_by(credit_refresh_interaction(
                OpenRouterCreditSource::StrictBudget,
            ))
    }

    fn model_rankings_indicator<'view>(
        &'view self,
        rankings_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        rankings_indicator
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
    let budget = &config.indicator_groups.openrouter_credit;
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
                .indicator_groups
                .openrouter_credit
                .max_blended_usd_per_million,
        },
    }
}
