use sottomano_desktop_sources::{SnapshotSource, ai_usage_spend::claude_usage::ClaudeCcusageBlock};

use crate::{
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        amount_text::spend_text,
        localized_quantities::Quantity,
        spend_to_level::level_from_cost,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(crate) const CLAUDE_CCUSAGE_BLOCK_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::Timer(Weight::Regular));

pub fn claude_ccusage_block_indicator(block: &ClaudeCcusageBlock) -> GroupIndicator<'_> {
    let (label, state, panel_visibility) = match &block.active {
        Some(active) => (
            ReadoutLabel::Labeled(format!("{} block", spend_text(&active.cost))),
            level_from_cost(&active.cost).into(),
            PanelVisibility::Shown,
        ),
        None => (
            ReadoutLabel::Unlabeled,
            IndicatorState::Idle,
            PanelVisibility::Hidden,
        ),
    };
    GroupIndicator::glyph_only(
        ClaudeCcusageBlock::NAME,
        Phosphor::Timer(Weight::Regular).into(),
        state,
    )
    .with_readout(IndicatorReadout {
        label,
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_panel_visibility(panel_visibility)
    .with_hover_card(Box::new(move |clock| {
        claude_ccusage_block_hover_card(block, clock)
    }))
}

pub fn claude_ccusage_block_hover_card(block: &ClaudeCcusageBlock, clock: Clock) -> Vec<Section> {
    let source = Section::new(
        "Cost Estimate",
        dictionary_table([
            LabeledValue::new("Source", "ccusage, local Claude Code logs"),
            LabeledValue::new("Cost Basis", "API list prices"),
        ]),
    );
    let Some(active) = &block.active else {
        return vec![source];
    };
    vec![
        source,
        Section::new(
            "Active 5-Hour Block",
            dictionary_table(
                [
                    Some(LabeledValue::new("Cost", Quantity::spend(active.cost))),
                    active
                        .cost_per_hour
                        .map(|rate| LabeledValue::new("Burn Rate", format!("{rate}/h"))),
                    active
                        .projected_cost
                        .map(|cost| LabeledValue::new("Projected Cost", Quantity::spend(cost))),
                    Some(LabeledValue::new(
                        "Ends In",
                        ValueCell::duration(active.ends_at - clock.utc()),
                    )),
                ]
                .into_iter()
                .flatten(),
            ),
        ),
    ]
}
