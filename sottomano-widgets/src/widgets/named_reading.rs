use chrono::Locale;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        icon_glyph::IconGlyph,
        tooltip_body::TooltipBody,
        widget_state::WidgetState,
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

    fn state(&self, clock: Clock) -> WidgetState {
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

    fn visible_in_bar(&self) -> bool {
        self.reading.visible_in_bar()
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        self.reading.tooltip(clock)
    }
}
