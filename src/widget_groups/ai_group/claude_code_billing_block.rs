use chrono::Utc;

use crate::{
    data::source::ai::claude::ClaudeCcusageBlock,
    widget_groups::ai_group::{
        amount_text::{countdown, dollars},
        spend_to_level::level_from_cost,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, Occasional},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeCcusageBlock {
    fn name(&self) -> &'static str {
        "Claude Code billing block"
    }

    fn icon(&self) -> Glyph {
        Phosphor::TerminalWindow(self.state().weight()).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::TerminalWindow(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.active.as_ref().map(|block| dollars(block.cost))
    }

    fn state(&self) -> WidgetState {
        self.active.as_ref().map_or(WidgetState::Idle, |block| {
            level_from_cost(block.cost).into()
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let source = Section::untitled_dictionary([
            KeyValue::new("Source", "ccusage, local Claude Code logs"),
            KeyValue::new("Cost Basis", "API list prices"),
        ]);
        let Some(block) = &self.active else {
            return TooltipBody::sections(&[source]);
        };
        TooltipBody::sections(&[
            source,
            Section::dictionary(
                "Active 5-Hour Block",
                [
                    Some(KeyValue::new("Cost", dollars(block.cost))),
                    block
                        .cost_per_hour
                        .map(|rate| KeyValue::new("Burn Rate", format!("{}/h", dollars(rate)))),
                    block
                        .projected_cost
                        .map(|cost| KeyValue::new("Projected Cost", dollars(cost))),
                    Some(KeyValue::new(
                        "Ends In",
                        countdown(block.ends_at, Utc::now()),
                    )),
                ]
                .into_iter()
                .flatten(),
            ),
        ])
    }
}

impl Occasional for ClaudeCcusageBlock {
    fn present(&self) -> bool {
        self.active.is_some()
    }
}
