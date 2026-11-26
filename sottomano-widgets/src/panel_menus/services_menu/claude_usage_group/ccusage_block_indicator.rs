use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::{ccusage_cli_reports::BlockProjection, claude_usage::ClaudeCcusageBlock},
};

use super::{ClaudeLocalSource, ClaudeUsageGroup, ClaudeUsageInteraction};
use crate::{
    clock::Clock,
    glyph_icons::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
    },
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
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
    let (label, state, panel_visibility) = match block {
        ClaudeCcusageBlock::Active(active) => (
            ReadoutLabel::Labeled(format!("{} block", spend_text(&active.cost))),
            level_from_cost(&active.cost).into(),
            PanelVisibility::Shown,
        ),
        ClaudeCcusageBlock::NoActiveBlock => (
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
    let ClaudeCcusageBlock::Active(active) = block else {
        return vec![source];
    };
    let mut block_rows = vec![LabeledValue::new("Cost", Quantity::spend(active.cost))];
    if let BlockProjection::Projected {
        cost_per_hour,
        projected_cost,
    } = active.projection
    {
        block_rows.push(LabeledValue::new("Burn Rate", format!("{cost_per_hour}/h")));
        block_rows.push(LabeledValue::new(
            "Projected Cost",
            Quantity::spend(projected_cost),
        ));
    }
    block_rows.push(LabeledValue::new(
        "Ends In",
        ValueCell::duration(active.ends_at - clock.utc()),
    ));
    vec![
        source,
        Section::new("Active 5-Hour Block", dictionary_table(block_rows)),
    ]
}

impl ClaudeUsageGroup {
    pub(super) fn ccusage_block_listing(&self) -> IndicatorListing<'_> {
        self.ccusage_block
            .indicator_listing(
                |held_snapshot| claude_ccusage_block_indicator(held_snapshot).when_shown(),
                CLAUDE_CCUSAGE_BLOCK_FALLBACK_GLYPH,
            )
            .map_indicators(|row| self.ccusage_block_indicator(row))
    }

    fn ccusage_block_indicator<'view>(
        &'view self,
        block_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        block_indicator
            .with_hover_card(Box::new(move |card_clock| {
                self.ccusage_block.sections_described_by(|snapshot| {
                    claude_ccusage_block_hover_card(snapshot, card_clock)
                })
            }))
            .refreshed_by(ClaudeUsageInteraction::Refresh(
                ClaudeLocalSource::CcusageBlock,
            ))
    }
}
