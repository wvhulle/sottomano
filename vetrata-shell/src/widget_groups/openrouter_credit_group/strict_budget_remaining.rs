use vetrata_config::severity::Severity;

use crate::{
    data::{
        money::Money,
        source::ai::openrouter::{KeyLimit, LimitReset, OpenRouterStrictBudget},
    },
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
        Phosphor::Wallet(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Wallet(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{} left", spend_text(&self.tightest_remaining())))
    }

    fn state(&self) -> WidgetState {
        if self.exhausted() {
            WidgetState::Critical
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(
            [Section::dictionary([
                remaining_entry(self.credits.remaining()),
                KeyValue::new("Lifetime Purchased", self.credits.total.to_string()),
            ])
            .titled("Credits")]
            .into_iter()
            .chain(self.key_limits.iter().map(KeyLimit::section)),
        )
    }

    fn visible_in_bar(&self) -> bool {
        self.credits.total.is_positive() || !self.key_limits.is_empty()
    }
}

impl KeyLimit {
    fn section(&self) -> Section {
        Section::dictionary(
            [
                Some(KeyValue::new("Limit", self.limit.to_string())),
                Some(remaining_entry(self.remaining)),
                self.reset
                    .map(|reset| KeyValue::new("Resets", reset.period_name())),
            ]
            .into_iter()
            .flatten(),
        )
        .titled((self.name.as_str(), "API key limit"))
    }
}

impl LimitReset {
    fn period_name(self) -> &'static str {
        match self {
            LimitReset::Daily => "daily",
            LimitReset::Weekly => "weekly",
            LimitReset::Monthly => "monthly",
        }
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
