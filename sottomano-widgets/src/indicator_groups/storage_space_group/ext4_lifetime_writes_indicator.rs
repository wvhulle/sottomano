use chrono::Locale;
use sottomano_desktop_sources::disk_storage::ext4_sysfs_counters::Ext4Counters;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

pub(super) struct Ext4LifetimeWritesIndicator {
    pub counters: Ext4Counters,
}

impl Ext4LifetimeWritesIndicator {
    pub(super) fn indicator<'indicator>(self, locale: Locale) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            "Writes",
            Phosphor::PencilSimple(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(
                Quantity::size(self.counters.lifetime_written).localized_text(locale),
            ),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_panel_visibility(PanelVisibility::Hidden)
        .with_hover_card(Box::new(move |_| self.hover_card()))
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
