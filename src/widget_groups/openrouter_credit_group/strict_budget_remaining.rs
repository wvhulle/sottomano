use crate::{
    data::{
        severity::Severity,
        source::ai::openrouter::{KeyLimit, LimitReset, OpenRouterStrictBudget},
    },
    widgets::{
        amount_text::{dollars, to_cents},
        bar_widget::{BarWidget, Glyph, Occasional},
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
        Some(format!("{} left", dollars(self.tightest_remaining())))
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
                KeyValue::new(
                    "Lifetime Purchased",
                    format!("${} USD", to_cents(self.credits.total)),
                ),
            ])
            .titled("Credits")]
            .into_iter()
            .chain(self.key_limits.iter().map(KeyLimit::section)),
        )
    }
}

impl Occasional for OpenRouterStrictBudget {
    fn present(&self) -> bool {
        self.credits.total > 0.0 || !self.key_limits.is_empty()
    }
}

impl KeyLimit {
    fn section(&self) -> Section {
        Section::dictionary(
            [
                Some(KeyValue::new(
                    "Limit",
                    format!("${} USD", to_cents(self.limit)),
                )),
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

fn remaining_entry(remaining: f64) -> KeyValue {
    let entry = KeyValue::new("Remaining", format!("${} USD", to_cents(remaining)));
    if remaining <= 0.0 {
        entry.level(Severity::Critical)
    } else {
        entry
    }
}
