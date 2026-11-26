use std::convert::Infallible;

use chrono::Locale;
use iced::futures::Stream;
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
};

use super::{ClockGroup, ClockInteraction, ClockSource};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::agenda_menu::clock_group::TICK,
    widgets::{localized_time::LocalizedTimeFormat, value_cells::LabeledValue},
};

pub(super) const LOCAL_TIME_GLYPH: Phosphor = Phosphor::Clock(Weight::Regular);

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

impl ClockGroup {
    pub(super) fn local_time_listing(&self, clock: Clock, locale: Locale) -> IndicatorListing<'_> {
        self.time
            .indicator_listing(
                |_| vec![LocalTime::indicator(clock, locale)],
                LOCAL_TIME_GLYPH.into(),
            )
            .map_indicators(|time_row| self.wired_local_time_indicator(time_row))
    }

    pub(super) fn wired_local_time_indicator<'view>(
        &'view self,
        time_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        time_indicator
            .with_hover_card(Box::new(|card_clock| {
                self.time
                    .sections_described_by(|_| LocalTime::hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalTime))
    }
}
