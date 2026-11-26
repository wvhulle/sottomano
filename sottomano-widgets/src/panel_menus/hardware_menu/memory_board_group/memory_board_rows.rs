use chrono::Locale;

use super::{
    MemoryBoardGroup, MemoryBoardInteraction,
    memory_usage_indicator::{MEMORY_USAGE_GLYPH, memory_indicator},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::IndicatorGroupFamily,
    },
};

impl MemoryBoardGroup {
    pub fn group_content(&self, locale: Locale) -> IndicatorGroupContent<'_> {
        let headline_indicator = self.memory_usage_headline(locale);
        let indicator_listing = [
            self.memory_usage_listing(locale),
            self.memory_growth_listing(locale),
            self.stall_pressure_listing(locale),
            self.swap_activity_listing(locale),
            self.memory_slot_listing(locale),
            self.module_temperature_listing(),
        ]
        .into_iter()
        .collect();
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::MemoryBoard,
            headline_indicator,
            &self.expansion,
            MemoryBoardInteraction::Expansion,
            indicator_listing,
        )
    }

    fn memory_usage_headline(&self, locale: Locale) -> GroupIndicator<'_> {
        self.memory_usage_indicator(self.memory.indicator(
            |held_snapshot| memory_indicator(held_snapshot, locale),
            MEMORY_USAGE_GLYPH.into(),
        ))
        .with_headline_glyph(Phosphor::Database(Weight::Regular).into())
    }
}
