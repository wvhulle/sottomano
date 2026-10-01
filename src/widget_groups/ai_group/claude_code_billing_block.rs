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
        tooltip_body::{TooltipBody, row, section, untitled},
        widget_state::WidgetState,
    },
};

impl BarWidget for ClaudeCcusageBlock {
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
        let source = untitled(vec![
            row("Source", "ccusage, local Claude Code logs"),
            row("Cost Basis", "API list prices"),
        ]);
        let Some(block) = &self.active else {
            return TooltipBody::Sections(vec![source]);
        };
        let rows = [
            Some(row("Cost", dollars(block.cost))),
            block
                .cost_per_hour
                .map(|rate| row("Burn Rate", format!("{}/h", dollars(rate)))),
            block
                .projected_cost
                .map(|projected| row("Projected Cost", dollars(projected))),
            Some(row("Ends In", countdown(block.ends_at, Utc::now()))),
        ]
        .into_iter()
        .flatten()
        .collect();
        TooltipBody::Sections(vec![source, section("Active 5-Hour Block", rows)])
    }
}

impl Occasional for ClaudeCcusageBlock {
    fn present(&self) -> bool {
        self.active.is_some()
    }
}
