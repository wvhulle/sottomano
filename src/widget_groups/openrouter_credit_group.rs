mod model_spend_breakdown;
mod openrouter_soft_budget;
mod strict_budget_remaining;

use std::path::PathBuf;

use iced::{Subscription, Task};

use crate::{
    config::Config,
    data::source::{
        Delivery,
        ai::{
            openrouter::{OpenRouterAccount, OpenRouterSpend, OpenRouterStrictBudget},
            working_time_forecast::SpendBudget,
        },
    },
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::{ForcedRefresh, polling},
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
    SoftBudget(Delivery<OpenRouterSpend>),
    StrictBudget(Delivery<OpenRouterStrictBudget>),
    Expansion(ExpansionMessage),
}

impl OpenRouterCreditGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn new(management_key: Option<PathBuf>) -> Self {
        Self {
            spend: ContinuousReading::default(),
            strict_budget: ContinuousReading::default(),
            management_key,
            expansion: GroupExpansion::default(),
        }
    }

    pub fn subscription(
        &self,
        config: &Config,
        forced: Option<ForcedRefresh>,
    ) -> Subscription<OpenRouterCreditMessage> {
        Subscription::batch(self.management_key.clone().into_iter().flat_map(|key| {
            let budget = &config.widget_groups.openrouter_credit;
            let account = OpenRouterAccount {
                management_key: key.clone(),
                budget: SpendBudget::new(config, budget.monthly_budget_usd),
                daily_warning_usd: budget.daily_warning_usd,
            };
            [
                polling::<OpenRouterSpend>(account, forced)
                    .map(OpenRouterCreditMessage::SoftBudget),
                polling::<OpenRouterStrictBudget>(key, forced)
                    .map(OpenRouterCreditMessage::StrictBudget),
            ]
        }))
    }

    pub fn update(&mut self, message: OpenRouterCreditMessage) -> Task<OpenRouterCreditMessage> {
        match message {
            OpenRouterCreditMessage::SoftBudget(reading) => self.spend.receive(reading),
            OpenRouterCreditMessage::StrictBudget(reading) => self.strict_budget.receive(reading),
            OpenRouterCreditMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    fn model_breakdown(&self) -> Option<ModelSpendBreakdown<'_>> {
        self.spend
            .present_reading()
            .filter(|spend| !spend.month.models.is_empty())
            .map(ModelSpendBreakdown)
    }

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
                self.spend.present_quota_group_item(
                    OpenRouterCreditTarget::SoftBudget,
                    OpenRouterCreditMessage::Expansion(ExpansionMessage::Expand),
                ),
                self.strict_budget
                    .present_group_item(OpenRouterCreditTarget::StrictBudget),
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
        .preview(self.spend.present().map(PreviewedValue::Plain))
    }
}
