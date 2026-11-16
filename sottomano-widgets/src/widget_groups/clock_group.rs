use chrono::Locale;

mod local_date;
mod local_time;
mod zone_time;

use std::time::Duration;

use chrono_tz::Tz;
use iced::{Subscription, Task};
use sottomano_desktop_sources::SnapshotDelivery;
use sottomano_toml_config::Config;

use self::{local_date::LocalDate, local_time::LocalTime, zone_time::ZoneTime};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    widget_groups::{
        group_summary::GroupSummary,
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

pub const TICK: Duration = Duration::from_secs(1);

const LOCAL_TIME_GLYPH: Phosphor = Phosphor::Clock(Weight::Regular);
const LOCAL_DATE_GLYPH: Phosphor = Phosphor::CalendarBlank(Weight::Regular);
const ZONE_TIME_GLYPH: Phosphor = Phosphor::GlobeHemisphereWest(Weight::Regular);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockSource {
    LocalTime,
    LocalDate,
    ZoneTime(Tz),
}

#[derive(Debug, Clone, Copy)]
pub enum ClockInteraction {
    Expansion(WidgetGroupExpansionMessage),
    Refresh(ClockSource),
}

#[derive(Default)]
pub struct ClockGroup {
    time: SubscribedSnapshot<LocalTime>,
    date: SubscribedSnapshot<LocalDate>,
    zone_times: Vec<(Tz, SubscribedSnapshot<ZoneTime>)>,
    expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum ClockMessage {
    Time(SnapshotDelivery<LocalTime>),
    Date(SnapshotDelivery<LocalDate>),
    Zone(Tz, SnapshotDelivery<ZoneTime>),
}

impl ClockGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, message: ClockMessage, clock: Clock) -> Task<ClockMessage> {
        match message {
            ClockMessage::Time(reading) => self.time.receive(reading, clock),
            ClockMessage::Date(reading) => self.date.receive(reading, clock),
            ClockMessage::Zone(zone, reading) => {
                self.zone_time_mut(zone).receive(reading, clock);
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: ClockInteraction) -> Task<ClockMessage> {
        match interaction {
            ClockInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            ClockInteraction::Refresh(source) => Self::refresh(source),
        }
    }

    pub fn subscription(
        &self,
        config: &Config,
        demand: GroupSourceDemand,
    ) -> Subscription<ClockMessage> {
        let demand = demand.expanded_when(self.expansion.state());
        let zones = config.calendar.other_time_zones.iter().map(|zone| {
            demand.while_rows_shown(
                snapshot_subscription::<ZoneTime>(*zone)
                    .with(*zone)
                    .map(|(zone, reading)| ClockMessage::Zone(zone, reading)),
            )
        });
        Subscription::batch(
            [
                snapshot_subscription::<LocalTime>(()).map(ClockMessage::Time),
                snapshot_subscription::<LocalDate>(()).map(ClockMessage::Date),
            ]
            .into_iter()
            .chain(zones),
        )
    }

    fn refresh(source: ClockSource) -> Task<ClockMessage> {
        match source {
            ClockSource::LocalTime => first_delivery::<LocalTime>(()).map(ClockMessage::Time),
            ClockSource::LocalDate => first_delivery::<LocalDate>(()).map(ClockMessage::Date),
            ClockSource::ZoneTime(zone) => first_delivery::<ZoneTime>(zone)
                .map(move |reading| ClockMessage::Zone(zone, reading)),
        }
    }

    fn zone_time_mut(&mut self, zone: Tz) -> &mut SubscribedSnapshot<ZoneTime> {
        let position = self
            .zone_times
            .iter()
            .position(|(known, _)| *known == zone)
            .unwrap_or_else(|| {
                self.zone_times.push((zone, SubscribedSnapshot::default()));
                self.zone_times.len() - 1
            });
        &mut self.zone_times[position].1
    }

    pub fn group_content(&self, clock: Clock, locale: Locale) -> WidgetGroupContent<'_> {
        let group_summary = match &self.time {
            SubscribedSnapshot::SnapshotHeld { .. } => GroupSummary::summarising(
                LOCAL_TIME_GLYPH.into(),
                self.local_time_item(LocalTime::reading_row(clock, locale)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    LOCAL_TIME_GLYPH.into(),
                    self.local_time_item(
                        troubled_snapshot.shown_over(LocalTime::reading_row(clock, locale), clock),
                    ),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                LOCAL_TIME_GLYPH.into(),
                self.local_time_item(placeholder.reading_row(LOCAL_TIME_GLYPH.into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                GroupSummary::glyph(LOCAL_TIME_GLYPH.into(), ReadingDisplayState::Pending)
            }
            SubscribedSnapshot::SourceUnavailable => {
                GroupSummary::glyph(LOCAL_TIME_GLYPH.into(), ReadingDisplayState::Off)
            }
        };
        let mut rows = Vec::new();
        match &self.date {
            SubscribedSnapshot::SnapshotHeld { .. } => {
                let date_row = LocalDate::reading_row(clock, locale);
                if date_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.local_date_item(date_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let date_row =
                    troubled_snapshot.shown_over(LocalDate::reading_row(clock, locale), clock);
                if date_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(self.local_date_item(date_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(self.local_date_item(placeholder.reading_row(LOCAL_DATE_GLYPH.into())));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        for (zone, zone_time) in &self.zone_times {
            match zone_time {
                SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                    let zone_row = held_snapshot.reading_row(clock, locale);
                    if zone_row.panel_visibility == PanelVisibility::Shown {
                        rows.push(Self::zone_time_item(*zone, zone_time, zone_row));
                    }
                }
                SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                    let zone_row = troubled_snapshot.shown_over(
                        troubled_snapshot.held_snapshot.reading_row(clock, locale),
                        clock,
                    );
                    if zone_row.panel_visibility == PanelVisibility::Shown {
                        rows.push(Self::zone_time_item(*zone, zone_time, zone_row));
                    }
                }
                SubscribedSnapshot::NoSnapshot { placeholder } => {
                    rows.push(Self::zone_time_item(
                        *zone,
                        zone_time,
                        placeholder.reading_row(ZONE_TIME_GLYPH.into()),
                    ));
                }
                SubscribedSnapshot::AwaitingFirstSnapshot
                | SubscribedSnapshot::SourceUnavailable => {}
            }
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Clock,
            group_summary,
            &self.expansion,
            ClockInteraction::Expansion,
            rows,
        )
    }

    fn local_time_item<'a>(&'a self, time_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(time_row)
            .with_hover_card(Box::new(|card_clock| {
                self.time
                    .sections_described_by(|_| LocalTime::hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalTime))
    }

    fn local_date_item<'a>(&'a self, date_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(date_row)
            .with_hover_card(Box::new(|card_clock| {
                self.date
                    .sections_described_by(|_| LocalDate::hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalDate))
    }

    fn zone_time_item<'a>(
        zone: Tz,
        zone_time: &'a SubscribedSnapshot<ZoneTime>,
        zone_row: ReadingRow<'a>,
    ) -> GroupItem<'a> {
        GroupItem::reading(zone_row)
            .with_hover_card(Box::new(|card_clock| {
                zone_time.sections_described_by(|snapshot| snapshot.hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::ZoneTime(zone)))
    }
}
