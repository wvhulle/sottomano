mod credit_balance_remaining;
mod openrouter_period_spend;
mod top_model_spend;

use std::path::PathBuf;

use iced::{Subscription, Task};

use crate::{
    config::Config,
    data::source::{
        Delivery,
        ai::{
            openrouter::{OpenRouterAccount, OpenRouterCreditBalance, OpenRouterSpend},
            working_time_forecast::SpendBudget,
        },
    },
    desktop_shell::hover_tooltips::{GroupHandle, HoverTarget},
    message_subscriptions::{ForcedRefresh, polling},
    widget_groups::openrouter_credit_group::top_model_spend::TopModelSpend,
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
    Spend,
    CreditBalance,
    TopModel,
}

impl From<OpenRouterCreditTarget> for HoverTarget {
    fn from(target: OpenRouterCreditTarget) -> Self {
        HoverTarget::OpenRouterCredit(target)
    }
}

pub struct OpenRouterCreditGroup {
    spend: ContinuousReading<OpenRouterSpend>,
    credit_balance: ContinuousReading<OpenRouterCreditBalance>,
    management_key: Option<PathBuf>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum OpenRouterCreditMessage {
    Spend(Delivery<OpenRouterSpend>),
    CreditBalance(Delivery<OpenRouterCreditBalance>),
    Expansion(ExpansionMessage),
}

impl OpenRouterCreditGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn new(management_key: Option<PathBuf>) -> Self {
        Self {
            spend: ContinuousReading::default(),
            credit_balance: ContinuousReading::default(),
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
                budget: SpendBudget::new(config, Some(budget.monthly_budget_usd)),
                daily_warning_usd: budget.daily_warning_usd,
            };
            [
                polling::<OpenRouterSpend>(account, forced).map(OpenRouterCreditMessage::Spend),
                polling::<OpenRouterCreditBalance>(key, forced)
                    .map(OpenRouterCreditMessage::CreditBalance),
            ]
        }))
    }

    pub fn update(&mut self, message: OpenRouterCreditMessage) -> Task<OpenRouterCreditMessage> {
        match message {
            OpenRouterCreditMessage::Spend(reading) => self.spend.receive(reading),
            OpenRouterCreditMessage::CreditBalance(reading) => self.credit_balance.receive(reading),
            OpenRouterCreditMessage::Expansion(menu) => {
                self.expansion.update(menu);
            }
        }
        Task::none()
    }

    fn top_model(&self) -> Option<TopModelSpend<'_>> {
        let spend = self.spend.present_reading()?;
        spend.today.models.first().map(|top| TopModelSpend {
            top,
            daily_total: spend.today.cost(),
        })
    }

    pub fn tooltip_for(&self, target: OpenRouterCreditTarget) -> Option<Tooltip> {
        match target {
            OpenRouterCreditTarget::Spend => self.spend.tooltip(),
            OpenRouterCreditTarget::CreditBalance => self.credit_balance.tooltip(),
            OpenRouterCreditTarget::TopModel => {
                self.spend.tooltip_with(self.top_model()?.tooltip())
            }
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, OpenRouterCreditMessage> {
        GroupContent::new(
            &self.expansion,
            OpenRouterCreditMessage::Expansion,
            [
                self.spend.present_quota_group_item(
                    OpenRouterCreditTarget::Spend,
                    OpenRouterCreditMessage::Expansion(ExpansionMessage::Expand),
                ),
                self.credit_balance
                    .present_group_item(OpenRouterCreditTarget::CreditBalance),
                self.top_model()
                    .map(|top| GroupItem::reading(top, OpenRouterCreditTarget::TopModel)),
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
