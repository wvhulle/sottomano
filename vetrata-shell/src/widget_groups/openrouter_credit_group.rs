mod model_spend_breakdown;
mod openrouter_soft_budget;
mod strict_budget_remaining;

use std::path::PathBuf;

use iced::{Subscription, Task};

use crate::{
    config::Config,
    data::source::{
        SourceDelivery,
        ai::{
            openrouter::{OpenRouterAccount, OpenRouterSpend, OpenRouterStrictBudget},
            working_time_forecast::SpendBudget,
        },
    },
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::{first_delivery, source_subscription},
    widget_groups::openrouter_credit_group::model_spend_breakdown::ModelSpendBreakdown,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        simple_icons::SimpleIcon,
        tooltip_body::Tooltip,
        widget_group::{
            ExpansionMessage, GroupContent, GroupExpansion, HandleFace, PreviewedValue,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenRouterCreditTarget {
    SoftBudget,
    StrictBudget,
    ModelBreakdown,
}

impl From<OpenRouterCreditTarget> for HoverTarget {
    fn from(target: OpenRouterCreditTarget) -> Self {
        HoverTarget::OpenRouterCredit(target)
    }
}

pub struct OpenRouterCreditGroup {
    spend: ContinuousReading<OpenRouterSpend>,
    strict_budget: ContinuousReading<OpenRouterStrictBudget>,
    management_key: Option<PathBuf>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum OpenRouterCreditMessage {
    SoftBudget(SourceDelivery<OpenRouterSpend>),
    StrictBudget(SourceDelivery<OpenRouterStrictBudget>),
    Expansion(ExpansionMessage),
}

impl OpenRouterCreditGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    #[must_use]
    pub fn new(management_key: Option<PathBuf>) -> Self {
        Self {
            spend: ContinuousReading::default(),
            strict_budget: ContinuousReading::default(),
            management_key,
            expansion: GroupExpansion::default(),
        }
    }

    fn account(management_key: PathBuf, config: &Config) -> OpenRouterAccount {
        let budget = &config.widget_groups.openrouter_credit;
        OpenRouterAccount {
            management_key,
            budget: SpendBudget::new(config, budget.monthly_budget_usd),
            daily_warning_usd: budget.daily_warning_usd,
        }
    }

    pub fn refresh(
        &self,
        target: OpenRouterCreditTarget,
        config: &Config,
    ) -> Task<OpenRouterCreditMessage> {
        let Some(key) = self.management_key.clone() else {
            return Task::none();
        };
        match target {
            OpenRouterCreditTarget::SoftBudget | OpenRouterCreditTarget::ModelBreakdown => {
                first_delivery::<OpenRouterSpend>(Self::account(key, config))
                    .map(OpenRouterCreditMessage::SoftBudget)
            }
            OpenRouterCreditTarget::StrictBudget => first_delivery::<OpenRouterStrictBudget>(key)
                .map(OpenRouterCreditMessage::StrictBudget),
        }
    }

    pub fn subscription(&self, config: &Config) -> Subscription<OpenRouterCreditMessage> {
        Subscription::batch(self.management_key.clone().into_iter().flat_map(|key| {
            [
                source_subscription::<OpenRouterSpend>(Self::account(key.clone(), config))
                    .map(OpenRouterCreditMessage::SoftBudget),
                source_subscription::<OpenRouterStrictBudget>(key)
                    .map(OpenRouterCreditMessage::StrictBudget),
            ]
        }))
    }

    pub fn update(&mut self, message: OpenRouterCreditMessage) -> Task<OpenRouterCreditMessage> {
        match message {
            OpenRouterCreditMessage::SoftBudget(reading) => self.spend.receive_delivery(reading),
            OpenRouterCreditMessage::StrictBudget(reading) => {
                self.strict_budget.receive_delivery(reading);
            }
            OpenRouterCreditMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    fn model_breakdown(&self) -> Option<ModelSpendBreakdown<'_>> {
        self.spend
            .visible_reading()
            .filter(|spend| !spend.month.models.is_empty())
            .map(ModelSpendBreakdown)
    }

    #[must_use]
    pub fn tooltip_for(&self, target: OpenRouterCreditTarget) -> Option<Tooltip> {
        match target {
            OpenRouterCreditTarget::SoftBudget => self.spend.tooltip(),
            OpenRouterCreditTarget::StrictBudget => self.strict_budget.tooltip(),
            OpenRouterCreditTarget::ModelBreakdown => {
                self.spend.tooltip_with(self.model_breakdown()?.tooltip())
            }
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, OpenRouterCreditMessage> {
        GroupContent::new(
            &self.expansion,
            OpenRouterCreditMessage::Expansion,
            [
                self.spend.group_item(OpenRouterCreditTarget::SoftBudget),
                self.strict_budget
                    .group_item(OpenRouterCreditTarget::StrictBudget),
                self.model_breakdown().map(|breakdown| {
                    GroupItem::reading(breakdown, OpenRouterCreditTarget::ModelBreakdown)
                }),
            ]
            .into_iter()
            .flatten(),
        )
        .face(HandleFace::Icon {
            glyph: SimpleIcon::OpenRouter.into(),
            handle: GroupHandle::OpenRouterCredit,
        })
        .preview(self.spend.display().map(PreviewedValue::Plain))
    }
}
