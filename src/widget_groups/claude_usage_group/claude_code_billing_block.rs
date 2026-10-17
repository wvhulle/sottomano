use chrono::Utc;

use crate::{
    data::source::ai::claude::ClaudeCcusageBlock,
    widgets::{
        amount_text::{countdown, spend_text},
        bar_widget::{BarWidget, Glyph, Occasional},
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_cost,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeCcusageBlock {
    fn name(&self) -> &'static str {
        "Claude Code billing block"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Timer(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::Timer(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.active
            .as_ref()
            .map(|block| format!("{} block", spend_text(&block.cost)))
    }

    fn state(&self) -> WidgetState {
        self.active.as_ref().map_or(WidgetState::Idle, |block| {
            level_from_cost(&block.cost).into()
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let source = Section::dictionary([
            KeyValue::new("Source", "ccusage, local Claude Code logs"),
            KeyValue::new("Cost Basis", "API list prices"),
        ]);
        let Some(block) = &self.active else {
            return TooltipBody::sections([source]);
        };
        TooltipBody::sections([
            source,
            Section::dictionary(
                [
                    Some(KeyValue::new("Cost", block.cost.to_string())),
                    block
                        .cost_per_hour
                        .map(|rate| KeyValue::new("Burn Rate", format!("{rate}/h"))),
                    block
                        .projected_cost
                        .map(|cost| KeyValue::new("Projected Cost", cost.to_string())),
                    Some(KeyValue::new(
                        "Ends In",
                        countdown(block.ends_at, Utc::now()),
                    )),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Active 5-Hour Block"),
        ])
    }
}

impl Occasional for ClaudeCcusageBlock {
    fn present(&self) -> bool {
        self.active.is_some()
    }
}
