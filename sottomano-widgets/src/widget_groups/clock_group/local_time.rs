use std::convert::Infallible;

use chrono::Locale;
use iced::futures::Stream;
use sottomano_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
};

use crate::{
    clock::Clock,
    widget_groups::clock_group::TICK,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
        value_cells::LabeledValue,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime;

impl SnapshotSource for LocalTime {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "Local time";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(TICK).boundary_aligned_deliveries(|| LocalTime)
    }
}

impl GroupItemReading for LocalTime {
    fn name(&self) -> &'static str {
        "Local time"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Clock(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        Some(LocalizedTimeFormat::TimeOfDay.format(&clock.local(), locale))
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let local = clock.local();
        TooltipBody::dictionary([
            LabeledValue::new("UTC offset", local.format("%:z").to_string()),
            LabeledValue::new("Unix time", local.timestamp().to_string()),
        ])
    }
}
