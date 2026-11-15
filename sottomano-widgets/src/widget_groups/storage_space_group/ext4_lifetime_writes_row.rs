use chrono::Locale;
use sottomano_desktop_sources::disk_storage::ext4_sysfs_counters::Ext4Counters;

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(super) struct Ext4LifetimeWritesRow {
    pub counters: Ext4Counters,
}

impl Ext4LifetimeWritesRow {
    pub(super) fn reading_row(self, locale: Locale) -> ReadingRow<'static> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(
                    Quantity::size(self.counters.lifetime_written).localized_text(locale),
                ),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            panel_visibility: PanelVisibility::Hidden,
            ..ReadingRow::glyph_only(
                "Writes",
                Phosphor::PencilSimple(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub(super) fn hover_card(&self) -> Vec<Section> {
        vec![Section::new(
            "Totals",
            dictionary_table([
                LabeledValue::new(
                    "Written over lifetime",
                    Quantity::size(self.counters.lifetime_written),
                ),
                LabeledValue::new(
                    "Written since mount",
                    Quantity::size(self.counters.written_since_mount),
                ),
            ]),
        )]
    }
}
