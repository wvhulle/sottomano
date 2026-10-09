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
        tooltip_body::{Section, TooltipBody},
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
        let source = Section::untitled(&[
            ("Source", "ccusage, local Claude Code logs"),
            ("Cost Basis", "API list prices"),
        ]);
        let Some(block) = &self.active else {
            return TooltipBody::sections(&[source]);
        };
        TooltipBody::sections(&[
            source,
            Section::titled(
                "Active 5-Hour Block",
                &[
                    ("Cost", Some(dollars(block.cost))),
                    (
                        "Burn Rate",
                        block
                            .cost_per_hour
                            .map(|rate| format!("{}/h", dollars(rate))),
                    ),
                    ("Projected Cost", block.projected_cost.map(dollars)),
                    ("Ends In", Some(countdown(block.ends_at, Utc::now()))),
                ],
            ),
        ])
    }
}

impl Occasional for ClaudeCcusageBlock {
    fn present(&self) -> bool {
        self.active.is_some()
    }
}
