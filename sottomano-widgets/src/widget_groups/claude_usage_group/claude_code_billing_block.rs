use chrono::Locale;
use sottomano_desktop_sources::ai_usage_spend::claude_usage::ClaudeCcusageBlock;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        amount_text::spend_text,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        spend_to_level::level_from_cost,
        value_cells::{LabeledValue, ValueCell},
    },
};

impl GroupItemReading for ClaudeCcusageBlock {
    fn name(&self) -> &'static str {
        "Claude Code billing block"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Timer(Weight::Regular).into()
    }

    fn fallback_icon() -> IconGlyph {
        Phosphor::Timer(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.active
            .as_ref()
            .map(|block| format!("{} block", spend_text(&block.cost)))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.active
            .as_ref()
            .map_or(ReadingDisplayState::Idle, |block| {
                level_from_cost(&block.cost).into()
            })
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let source = Section::dictionary([
            LabeledValue::new("Source", "ccusage, local Claude Code logs"),
            LabeledValue::new("Cost Basis", "API list prices"),
        ]);
        let Some(block) = &self.active else {
            return HoverCardBody::sections([source]);
        };
        HoverCardBody::sections([
            source,
            Section::dictionary(
                [
                    Some(LabeledValue::new("Cost", Quantity::spend(block.cost))),
                    block
                        .cost_per_hour
                        .map(|rate| LabeledValue::new("Burn Rate", format!("{rate}/h"))),
                    block
                        .projected_cost
                        .map(|cost| LabeledValue::new("Projected Cost", Quantity::spend(cost))),
                    Some(LabeledValue::new(
                        "Ends In",
                        ValueCell::duration(block.ends_at - clock.utc()),
                    )),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Active 5-Hour Block"),
        ])
    }

    fn panel_visibility(&self) -> PanelVisibility {
        if self.active.is_some() {
            PanelVisibility::Shown
        } else {
            PanelVisibility::Hidden
        }
    }
}
