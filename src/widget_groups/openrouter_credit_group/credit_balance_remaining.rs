use crate::{
    data::source::ai::openrouter::OpenRouterCreditBalance,
    widgets::{
        amount_text::{dollars, to_cents},
        bar_widget::{BarWidget, Glyph, Occasional},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for OpenRouterCreditBalance {
    fn name(&self) -> &'static str {
        "OpenRouter credit balance"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Wallet(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Wallet(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{} left", dollars(self.remaining())))
    }

    fn state(&self) -> WidgetState {
        if self.overspent() {
            WidgetState::Critical
        } else {
            WidgetState::Idle
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Remaining", format!("${} USD", to_cents(self.remaining()))),
            KeyValue::new(
                "Lifetime Purchased",
                format!("${} USD", to_cents(self.0.total)),
            ),
        ])
        .titled("Credits")])
    }
}

impl Occasional for OpenRouterCreditBalance {
    fn present(&self) -> bool {
        self.0.total > 0.0
    }
}
