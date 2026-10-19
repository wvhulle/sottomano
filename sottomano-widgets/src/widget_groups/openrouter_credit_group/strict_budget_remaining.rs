use chrono::Locale;
use sottomano_config::severity::Severity;
use sottomano_sources::ai_usage_spend::{
    openrouter_credits::{KeyLimit, LimitReset, OpenRouterStrictBudget},
    spend_money::Money,
};

use crate::{
    clock::Clock,
    widgets::{
        amount_text::spend_text,
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for OpenRouterStrictBudget {
    fn name(&self) -> &'static str {
        "OpenRouter strict budget"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Wallet(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Wallet(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{} left", spend_text(&self.tightest_remaining())))
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.exhausted() {
            WidgetState::Critical
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections(
            [Section::dictionary([
                remaining_entry(self.credits.remaining()),
                KeyValue::new("Lifetime Purchased", self.credits.total.to_string()),
            ])
            .titled("Credits")]
            .into_iter()
            .chain(self.key_limits.iter().map(Section::from)),
        )
    }

    fn visible_in_bar(&self) -> bool {
        self.credits.total.is_positive() || !self.key_limits.is_empty()
    }
}

impl From<&KeyLimit> for Section {
    fn from(key_limit: &KeyLimit) -> Self {
        let resets = key_limit.reset.map(|reset| {
            let period_name = match reset {
                LimitReset::Daily => "daily",
                LimitReset::Weekly => "weekly",
                LimitReset::Monthly => "monthly",
            };
            KeyValue::new("Resets", period_name)
        });
        Section::dictionary(
            [
                Some(KeyValue::new("Limit", key_limit.limit.to_string())),
                Some(remaining_entry(key_limit.remaining)),
                resets,
            ]
            .into_iter()
            .flatten(),
        )
        .titled((key_limit.name.as_str(), "API key limit"))
    }
}

fn remaining_entry(remaining: Money) -> KeyValue {
    let entry = KeyValue::new("Remaining", remaining.to_string());
    if remaining.is_positive() {
        entry
    } else {
        entry.level(Severity::Critical)
    }
}
