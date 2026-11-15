use sottomano_desktop_sources::{
    SnapshotSource,
    ai_usage_spend::copilot_usage::{CopilotFree, USER_ENDPOINT},
};

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(crate) const COPILOT_FREE_FALLBACK_GLYPH: IconGlyph =
    IconGlyph::Phosphor(Phosphor::ChatCircleDots(Weight::Regular));

pub fn copilot_free_reading_row(free: &CopilotFree) -> ReadingRow<'_> {
    let state = if free.chat <= 5 {
        ReadingDisplayState::Critical
    } else if free.chat <= 20 {
        ReadingDisplayState::Warning
    } else {
        ReadingDisplayState::Idle
    };
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(format!("{} chats", free.chat)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            CopilotFree::NAME,
            Phosphor::ChatCircleDots(Weight::Regular).into(),
            state,
            Box::new(move |_| copilot_free_hover_card(free)),
        )
    }
}

pub fn copilot_free_hover_card(free: &CopilotFree) -> Vec<Section> {
    vec![
        Section::new(
            "Subscription",
            dictionary_table(
                free.plan
                    .as_deref()
                    .map(|plan| LabeledValue::new("Plan", plan))
                    .into_iter()
                    .chain([LabeledValue::new("Source", USER_ENDPOINT)]),
            ),
        ),
        Section::new(
            "Remaining",
            dictionary_table(
                [
                    LabeledValue::new("Chat", Quantity::count(free.chat)),
                    LabeledValue::new("Completions", Quantity::count(free.completions)),
                ]
                .into_iter()
                .chain(free.reset.map(|at| {
                    LabeledValue::new(
                        "Resets",
                        ValueCell::Time {
                            at,
                            format: LocalizedTimeFormat::Date,
                        },
                    )
                })),
            ),
        ),
    ]
}
