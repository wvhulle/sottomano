mod best_models_within_cap;
mod openrouter_soft_budget;
mod strict_budget_remaining;

use std::path::PathBuf;

use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SnapshotDelivery,
    ai_usage_spend::{
        openrouter_credits::{self, OpenRouterAccount, OpenRouterSpend, OpenRouterStrictBudget},
        openrouter_model_rankings::{ModelRankingInputs, OpenRouterModelRankings},
        working_time_forecast::{SpendBudget, SpendPacing},
    },
    network_connectivity::internet_connectivity_gate::OnlineSourceInputs,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::{GroupHandle, HoverTarget},
    widgets::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        latest_snapshot::LatestSnapshot,
        most_used_model::{ModelSpendRanking, MostUsedModel, MostUsedModelSpend, RankedSpan},
        simple_icons::SimpleIcon,
        tooltip_body::{Section, tooltip_source_delivery::Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, HandleFace, WidgetGroup},
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
    analysis_key: Option<PathBuf>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum OpenRouterCreditMessage {
    SoftBudget(SnapshotDelivery<OpenRouterSpend>),
    StrictBudget(SnapshotDelivery<OpenRouterStrictBudget>),
    ModelRankings(SnapshotDelivery<OpenRouterModelRankings>),
    Expansion(ExpansionMessage),
}

impl WidgetGroup for OpenRouterCreditGroup {
    const TITLE: &'static str = "OpenRouter credit";
}

impl OpenRouterCreditGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(management_key: Option<PathBuf>, analysis_key: Option<PathBuf>) -> Self {
        Self {
            spend: LatestSnapshot::default(),
            strict_budget: LatestSnapshot::default(),
            model_rankings: LatestSnapshot::default(),
            management_key,
            analysis_key,
            expansion: GroupExpansion::default(),
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
                self.analysis_key
                    .clone()
                    .map_or_else(Task::none, |analysis_key| {
                        first_delivery::<OpenRouterModelRankings>(ranking_inputs(
                            analysis_key,
                            config,
                            buses,
                        ))
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
        Subscription::batch(
            self.management_key
                .clone()
                .into_iter()
                .flat_map(|key| {
                    [
                        snapshot_subscription::<OpenRouterSpend>(account(
                            key.clone(),
                            config,
                            buses,
                        ))
                        .map(OpenRouterCreditMessage::SoftBudget),
                        snapshot_subscription::<OpenRouterStrictBudget>(online(key, buses))
                            .map(OpenRouterCreditMessage::StrictBudget),
                    ]
                })
                .chain(self.analysis_key.clone().map(|analysis_key| {
                    snapshot_subscription::<OpenRouterModelRankings>(ranking_inputs(
                        analysis_key,
                        config,
                        buses,
                    ))
                    .map(OpenRouterCreditMessage::ModelRankings)
                })),
        )
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
    pub fn tooltip_for(&self, target: OpenRouterCreditTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            OpenRouterCreditTarget::SoftBudget => self.spend.tooltip(clock),
            OpenRouterCreditTarget::StrictBudget => self.strict_budget.tooltip(clock),
            OpenRouterCreditTarget::BestModelsWithinCap => self.model_rankings.tooltip(clock),
            OpenRouterCreditTarget::MostUsedModel => self
                .spend
                .tooltip_with(MostUsedModel(self.model_ranking()?).tooltip(clock)),
            OpenRouterCreditTarget::MostUsedModelSpend => self
                .spend
                .tooltip_with(MostUsedModelSpend(self.model_ranking()?).tooltip(clock)),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, OpenRouterCreditMessage> {
        Self::expandable_group(
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
        .face(HandleFace::Icon {
            glyph: SimpleIcon::OpenRouter.into(),
            handle: GroupHandle::OpenRouterCredit,
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
    analysis_key: PathBuf,
    config: &Config,
    buses: &DesktopBuses,
) -> OnlineSourceInputs<ModelRankingInputs> {
    online(
        ModelRankingInputs {
            analysis_key,
            max_blended_per_million: config
                .widget_groups
                .openrouter_credit
                .max_blended_usd_per_million,
        },
        buses,
    )
}
