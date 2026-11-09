use chrono::Locale;
use sottomano_desktop_sources::disk_storage::ext4_sysfs_counters::Ext4Counters;

use crate::{
    clock::Clock,
    group_items::group_item_reading::{GroupItemReading, PanelVisibility},
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub(super) struct Ext4LifetimeWritesRow {
    pub counters: Ext4Counters,
}

impl GroupItemReading for Ext4LifetimeWritesRow {
    fn name(&self) -> &'static str {
        "Writes"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::PencilSimple(Weight::Regular).into()
    }

    fn panel_visibility(&self) -> PanelVisibility {
        PanelVisibility::Hidden
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(Quantity::size(self.counters.lifetime_written).localized_text(locale))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new(
                "Written over lifetime",
                Quantity::size(self.counters.lifetime_written),
            ),
            LabeledValue::new(
                "Written since mount",
                Quantity::size(self.counters.written_since_mount),
            ),
        ])
    }
}
