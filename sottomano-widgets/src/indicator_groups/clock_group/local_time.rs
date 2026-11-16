use std::convert::Infallible;

use chrono::Locale;
use iced::futures::Stream;
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
};

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    indicator_groups::clock_group::TICK,
    widgets::{
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
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

impl LocalTime {
    pub(super) fn indicator<'indicator>(
        clock: Clock,
        locale: Locale,
    ) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            Self::NAME,
            Phosphor::Clock(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(
                LocalizedTimeFormat::TimeOfDay.format(&clock.local(), locale),
            ),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(Self::hover_card))
    }

    pub(super) fn hover_card(clock: Clock) -> Vec<Section> {
        let local = clock.local();
        vec![Section::new(
            "Relative to UTC",
            dictionary_table([
                LabeledValue::new("UTC offset", local.format("%:z").to_string()),
                LabeledValue::new("Unix time", local.timestamp().to_string()),
            ]),
        )]
    }
}
