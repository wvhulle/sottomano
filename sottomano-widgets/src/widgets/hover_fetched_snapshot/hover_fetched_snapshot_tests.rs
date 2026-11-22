use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    time::SystemTime,
};

use iced::futures::Stream;
use sottomano_desktop_sources::delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling};

use super::*;
use crate::hover_cards::hover_card_body::{
    SectionWidget, hover_card_source_delivery::SectionSource,
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
    type StreamInputs = u32;

    const NAME: &'static str = "Counted snapshot";

    fn delivery_stream(_inputs: u32) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::empty()
    }
}

const POLL_PERIOD: Duration = Duration::from_mins(10);

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

fn polled_snapshot(count: u32) -> SnapshotDelivery<CountedSnapshot> {
    SnapshotDelivery::fixture_delivered(
        Ok(CountedSnapshot {
            snapshot_count: count,
        }),
        DeliveryCadence::PeriodicPolling(PeriodicPolling::every(POLL_PERIOD)),
    )
}

fn hovered_at(hover: &mut HoverFetchedSnapshot<CountedSnapshot>, window: u32, clock: Clock) {
    let _ = hover.start_hover_fetch(window, |_| (), &Config::default(), clock);
}

fn delivery_state(hover: &HoverFetchedSnapshot<CountedSnapshot>) -> SourceDelivery {
    let sections = hover.sections_described_by(|_| {
        vec![Section::new("Count", SectionWidget::paragraph("counted"))]
    });
    let Some(SectionSource::Snapshot { delivery, .. }) =
        sections.first().map(|section| section.source)
    else {
        panic!("a hover fetch in progress describes its source");
    };
    delivery
}

#[test]
fn a_first_hover_shows_a_loading_state_without_a_cadence() {
    let timeline = TestTimeline::starting_now();
    let mut hover = HoverFetchedSnapshot::default();
    hovered_at(&mut hover, 1, timeline.at(0));
    assert_eq!(delivery_state(&hover), SourceDelivery::Loading);
    assert!(hover.sample().is_none());
    let seeded = hover.sections_described_by(|_| Vec::new());
    assert_eq!(
        seeded
            .iter()
            .map(|section| (section.title.text().to_owned(), section.source))
            .collect::<Vec<_>>(),
        [(
            "Loading".to_owned(),
            SectionSource::Snapshot {
                source_name: "Counted snapshot",
                delivery: SourceDelivery::Loading,
            }
        )]
    );
}

#[test]
fn a_rehover_within_the_poll_period_serves_the_remembered_snapshot() {
    let timeline = TestTimeline::starting_now();
    let mut hover = HoverFetchedSnapshot::default();
    hovered_at(&mut hover, 1, timeline.at(0));
    hover.receive(polled_snapshot(4), timeline.at(0));
    hover.stop();
    hovered_at(&mut hover, 1, timeline.at(60));
    assert_eq!(hover.sample(), Some(&CountedSnapshot { snapshot_count: 4 }));
    assert_eq!(
        delivery_state(&hover),
        SourceDelivery::Delivered(DeliveryStamp {
            cadence: DeliveryCadence::PeriodicPolling(PeriodicPolling::every(POLL_PERIOD)),
            delivered_at: timeline.at(0).instant,
        })
    );
}

#[test]
fn a_rehover_after_the_poll_period_loads_while_showing_the_remembered_snapshot() {
    let timeline = TestTimeline::starting_now();
    let mut hover = HoverFetchedSnapshot::default();
    hovered_at(&mut hover, 1, timeline.at(0));
    hover.receive(polled_snapshot(4), timeline.at(0));
    hover.stop();
    hovered_at(&mut hover, 1, timeline.at(601));
    assert_eq!(hover.sample(), Some(&CountedSnapshot { snapshot_count: 4 }));
    assert_eq!(delivery_state(&hover), SourceDelivery::Loading);
}

#[test]
fn another_window_never_shows_the_previous_windows_snapshot() {
    let timeline = TestTimeline::starting_now();
    let mut hover = HoverFetchedSnapshot::default();
    hovered_at(&mut hover, 1, timeline.at(0));
    hover.receive(polled_snapshot(4), timeline.at(0));
    hover.stop();
    hovered_at(&mut hover, 2, timeline.at(30));
    assert!(hover.sample().is_none());
    assert_eq!(delivery_state(&hover), SourceDelivery::Loading);
}

#[test]
fn an_unavailable_source_forgets_its_remembered_snapshot() {
    let timeline = TestTimeline::starting_now();
    let mut hover = HoverFetchedSnapshot::default();
    hovered_at(&mut hover, 1, timeline.at(0));
    hover.receive(polled_snapshot(4), timeline.at(0));
    hover.receive(SnapshotDelivery::SourceUnavailable, timeline.at(3));
    hover.stop();
    hovered_at(&mut hover, 1, timeline.at(30));
    assert!(hover.sample().is_none());
}
