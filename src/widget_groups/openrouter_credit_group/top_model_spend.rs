use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    data::source::ai::openrouter::ModelSpend,
    widgets::{
        amount_text::{dollars, percent_text, to_cents},
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        spend_to_level::level_from_cost,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct TopModelSpend<'a> {
    pub top: &'a ModelSpend,
    pub daily_total: f64,
}

impl BarWidget for TopModelSpend<'_> {
    fn name(&self) -> &'static str {
        "OpenRouter top model"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Sparkle(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{} {}", self.top.model, dollars(self.top.cost)))
    }

    fn state(&self) -> WidgetState {
        level_from_cost(self.top.cost).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::dictionary([
            KeyValue::new("Model", self.top.model.as_str()),
            KeyValue::new("Today", format!("${} USD", to_cents(self.top.cost))),
            KeyValue::new(
                "Share of Today",
                percent_text(Ratio::new::<ratio>(self.top.cost / self.daily_total)),
            ),
        ])
        .titled("Most Expensive Model Today")])
    }
}
