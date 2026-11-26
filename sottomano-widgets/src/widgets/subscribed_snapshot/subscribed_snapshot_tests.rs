use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    time::{Duration, SystemTime},
};

use iced::futures::{Stream, stream};
use sottomano_desktop_sources::delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling};

use super::*;
use crate::{
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
};

#[derive(Debug, Clone, PartialEq)]
struct CountedSnapshot {
    snapshot_count: u32,
}

#[derive(Debug, Clone)]
struct CountingFailure;

impl Display for CountingFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "counting failed")
    }
}

impl SnapshotSource for CountedSnapshot {
    type Error = CountingFailure;
    type StreamInputs = ();

    const NAME: &'static str = "Counted snapshot";

    fn delivery_stream(_inputs: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::empty()
    }
}

fn counted_row(_snapshot: &CountedSnapshot) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only("Counted", IconGlyph::default(), IndicatorState::Idle)
        .with_hover_card(Box::new(|_| Vec::new()))
}

fn shown_state(latest: &SubscribedSnapshot<CountedSnapshot>) -> Option<IndicatorState> {
    latest.shown_state(
        |snapshot| counted_row(snapshot).shown_state(),
        |_| PanelVisibility::Shown,
    )
}

fn delivered_at(latest: &SubscribedSnapshot<CountedSnapshot>) -> Option<DeliveryStamp> {
    match latest {
        SubscribedSnapshot::Available { delivered_at, .. } => Some(*delivered_at),
        SubscribedSnapshot::Unavailable { .. } => None,
    }
}

struct TestTimeline {
    started: Clock,
}

impl TestTimeline {
    fn starting_now() -> Self {
        Self {
            started: Clock::real(),
        }
    }

    fn at(&self, seconds_since_epoch: u64) -> Clock {
        let elapsed = Duration::from_secs(seconds_since_epoch);
        Clock {
            instant: self.started.instant + elapsed,
            wall: SystemTime::UNIX_EPOCH + elapsed,
        }
    }
}

fn polled_delivery(
    snapshot_fetch: Result<CountedSnapshot, CountingFailure>,
) -> SnapshotDelivery<CountedSnapshot> {
    SnapshotDelivery::fixture_delivered(
        snapshot_fetch,
        DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(3))),
    )
}

#[test]
fn a_fresh_latest_snapshot_awaits_its_first_delivery() {
    let latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    assert!(latest.awaits_first_delivery());
    assert!(delivered_at(&latest).is_none());
}

#[test]
fn an_unchanged_snapshot_keeps_its_poll_period_and_moves_its_delivery_time() {
    let timeline = TestTimeline::starting_now();
    let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 1 })),
        timeline.at(10),
    );
    latest.receive(SnapshotDelivery::SnapshotUnchanged, timeline.at(13));
    assert_eq!(
        delivered_at(&latest),
        Some(DeliveryStamp {
            cadence: DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(
                3
            ))),
            delivered_at: timeline.at(13).instant,
        })
    );
}

#[test]
fn a_failed_fetch_keeps_the_held_snapshot_and_marks_it_failed() {
    let timeline = TestTimeline::starting_now();
    let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 1 })),
        timeline.at(10),
    );
    latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(13));
    assert_eq!(latest.get(), Some(&CountedSnapshot { snapshot_count: 1 }));
    assert_eq!(shown_state(&latest), Some(IndicatorState::Failed));
}

#[test]
fn a_failure_before_any_snapshot_shows_a_failed_placeholder() {
    let timeline = TestTimeline::starting_now();
    let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(10));
    assert!(latest.failure().is_some());
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 2 })),
        timeline.at(13),
    );
    latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(16));
    assert_eq!(latest.get(), Some(&CountedSnapshot { snapshot_count: 2 }));
    assert!(latest.failure().is_none());
}

#[test]
fn a_command_before_any_delivery_changes_nothing() {
    let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    latest.apply_command(CommandOutcome::Applied {
        expected_snapshot: CountedSnapshot { snapshot_count: 5 },
    });
    assert!(latest.awaits_first_delivery());
}

#[test]
fn a_refused_command_drops_the_assumed_snapshot_and_shows_its_failure() {
    let timeline = TestTimeline::starting_now();
    let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 1 })),
        timeline.at(10),
    );
    latest.apply_command(CommandOutcome::Refused {
        refusal: CountingFailure,
    });
    assert!(latest.failure().is_some());
    assert!(latest.get().is_none());
}

#[test]
fn a_vanished_source_drops_its_snapshot_and_awaits_again() {
    let timeline = TestTimeline::starting_now();
    let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 1 })),
        timeline.at(10),
    );
    latest.receive(SnapshotDelivery::SourceUnavailable, timeline.at(11));
    assert!(latest.awaits_first_delivery());
    assert_eq!(shown_state(&latest), None);
}

fn listed_and_unread_counts(latest: &SubscribedSnapshot<CountedSnapshot>) -> [usize; 2] {
    let listing = latest.indicator_listing(
        |snapshot| {
            (0..snapshot.snapshot_count)
                .map(|_| counted_row(snapshot))
                .collect()
        },
        IconGlyph::default(),
    );
    [
        listing.listed_indicators().len(),
        listing.unread_sources().len(),
    ]
}

#[test]
fn a_reading_with_entries_is_listed() {
    let timeline = TestTimeline::starting_now();
    let mut latest = SubscribedSnapshot::default();
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 2 })),
        timeline.at(10),
    );
    assert_eq!(listed_and_unread_counts(&latest), [2, 0]);
}

#[test]
fn a_reading_without_entries_lists_nothing() {
    let timeline = TestTimeline::starting_now();
    let mut latest = SubscribedSnapshot::default();
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 0 })),
        timeline.at(10),
    );
    let listing = latest.indicator_listing(|_| Vec::new(), IconGlyph::default());
    assert!(matches!(listing, IndicatorListing::Listed(listed) if listed.is_empty()));
}

#[test]
fn a_source_still_loading_is_an_unread_source() {
    let latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    assert_eq!(listed_and_unread_counts(&latest), [0, 1]);
}

#[test]
fn a_failed_first_reading_is_an_unread_source() {
    let timeline = TestTimeline::starting_now();
    let mut latest = SubscribedSnapshot::default();
    latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(10));
    assert_eq!(listed_and_unread_counts(&latest), [0, 1]);
}

#[test]
fn an_absent_source_lists_nothing() {
    let timeline = TestTimeline::starting_now();
    let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
    latest.receive(SnapshotDelivery::SourceUnavailable, timeline.at(10));
    let listing = latest.indicator_listing(|_| Vec::new(), IconGlyph::default());
    assert!(matches!(listing, IndicatorListing::Listed(listed) if listed.is_empty()));
}

#[test]
fn a_stale_empty_reading_keeps_its_failure_visible() {
    let timeline = TestTimeline::starting_now();
    let mut latest = SubscribedSnapshot::default();
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 0 })),
        timeline.at(10),
    );
    latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(13));
    assert_eq!(listed_and_unread_counts(&latest), [0, 1]);
}

#[test]
fn stale_entries_stay_listed_in_a_failed_state() {
    let timeline = TestTimeline::starting_now();
    let mut latest = SubscribedSnapshot::default();
    latest.receive(
        polled_delivery(Ok(CountedSnapshot { snapshot_count: 1 })),
        timeline.at(10),
    );
    latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(13));
    let listing =
        latest.indicator_listing(|snapshot| vec![counted_row(snapshot)], IconGlyph::default());
    assert_eq!(
        listing
            .listed_indicators()
            .iter()
            .map(GroupIndicator::shown_state)
            .collect::<Vec<_>>(),
        [IndicatorState::Failed]
    );
}
