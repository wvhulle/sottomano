use crate::{
    group_indicators::indicator_group_content::IndicatorListing,
    panel_menus::services_menu::openrouter_credit_group::spend_indicator::spend_indicator,
};
mod best_models_indicator;
mod most_used_model_indicator;
mod spend_indicator;
mod strict_budget_indicator;

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
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
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

    pub fn group_content(&self, clock: Clock) -> IndicatorGroupContent<'_> {
        let headline_indicator = self
            .wired_spend_indicator(self.spend.indicator(
                |held_snapshot| spend_indicator(held_snapshot, clock),
                Phosphor::Coins(Weight::Regular).into(),
            ))
            .with_headline_glyph(SimpleIcon::OpenRouter.into());
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::OpenRouterCredit,
            headline_indicator,
            &self.expansion,
            OpenRouterCreditInteraction::Expansion,
            [
                self.spend_listing(clock),
                self.strict_budget_listing(),
                self.best_models_listing(),
                IndicatorListing::Listed(self.most_used_model_rows()),
            ]
            .into_iter()
            .collect(),
        )
    }
}

pub(super) fn credit_refresh_interaction(
    source: OpenRouterCreditSource,
) -> OpenRouterCreditInteraction {
    OpenRouterCreditInteraction::OnBuses(OpenRouterCreditBusRequest::Refresh(source))
}

fn account(
    management_key: PathBuf,
    config: &Config,
    buses: &DesktopBuses,
) -> OnBus<OpenRouterAccount> {
    let budget = &config.panel_menus.services.openrouter_credit;
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
                .panel_menus
                .services
                .openrouter_credit
                .max_blended_usd_per_million,
        },
    }
}
