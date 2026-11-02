use chrono::Locale;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        hover_card_body::HoverCardBody,
        icon_glyph::IconGlyph,
        reading_display_state::ReadingDisplayState,
    },
};

pub struct NamedReading<'a> {
    pub reading: &'a dyn GroupItemReading,
    pub shown_name: &'a str,
}

impl GroupItemReading for NamedReading<'_> {
    fn name(&self) -> &'static str {
        self.reading.name()
    }

    fn icon(&self) -> IconGlyph {
        self.reading.icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        Some(match self.reading.label(clock, locale) {
            Some(label) => format!("{} {label}", self.shown_name),
            None => self.shown_name.to_owned(),
        })
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        self.reading.state(clock)
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        self.reading.ongoing_toast_threshold()
    }

    fn ongoing_toast_body(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.label(clock, locale)
    }

    fn gauge(&self) -> Option<Ratio> {
        self.reading.gauge()
    }

    fn live_gauge(&self) -> Option<Ratio> {
        self.reading.live_gauge()
    }

    fn visible_on_panel(&self) -> bool {
        self.reading.visible_on_panel()
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        self.reading.hover_card(clock)
    }
}
