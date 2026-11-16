use std::{collections::HashMap, mem::replace, time::Duration};

use iced::{
    Task,
    futures::{StreamExt, stream},
    task::Handle,
};
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::internet_connectivity_gate::InternetUnreachable,
};
use sottomano_toml_config::Config;
use tokio::time::sleep;

use crate::{
    clock::Clock,
    hover_cards::hover_card_body::{
        Section,
        hover_card_source_delivery::{DeliveryStamp, RefreshDeadline, SourceDelivery},
    },
};

#[derive(Debug, Clone)]
pub struct RememberedSnapshot<S: SnapshotSource> {
    remembered_snapshot: S,
    delivery_timing: DeliveryStamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefetchSchedule {
    FetchNow,
    FetchAfter(Duration),
}

#[derive(Debug)]
pub enum HoverFetchProgress<S: SnapshotSource> {
    ServingRemembered,
    AwaitingFirstDelivery,
    SourceUnavailable,
    InternetUnreachable(InternetUnreachable),
    Delivered {
        snapshot_fetch: Result<S, S::Error>,
        delivery_timing: DeliveryStamp,
    },
}

#[derive(Debug)]
pub enum SnapshotFetchStage<S: SnapshotSource> {
    Idle,
    Fetching {
        inputs: S::StreamInputs,
        fetch_handle: Handle,
        progress: HoverFetchProgress<S>,
    },
}

#[derive(Debug)]
pub struct HoverFetchedSnapshot<S: SnapshotSource> {
    remembered_by_inputs: HashMap<S::StreamInputs, RememberedSnapshot<S>>,
    hover_fetch: SnapshotFetchStage<S>,
}

impl<S: SnapshotSource> Default for HoverFetchedSnapshot<S> {
    fn default() -> Self {
        Self {
            remembered_by_inputs: HashMap::new(),
            hover_fetch: SnapshotFetchStage::Idle,
        }
    }
}

impl<S: SnapshotSource> RememberedSnapshot<S> {
    fn refetch_schedule(&self, clock: Clock) -> RefetchSchedule {
        let RefreshDeadline::DueAt { deadline, .. } = self.delivery_timing.refresh_deadline()
        else {
            return RefetchSchedule::FetchNow;
        };
        match deadline.checked_duration_since(clock.instant) {
            Some(remaining) if !remaining.is_zero() => RefetchSchedule::FetchAfter(remaining),
            Some(_) | None => RefetchSchedule::FetchNow,
        }
    }
}

impl<S: SnapshotSource> HoverFetchProgress<S> {
    fn delivered(self, delivery: SnapshotDelivery<S>, clock: Clock) -> Self {
        match delivery {
            SnapshotDelivery::SourceUnavailable => HoverFetchProgress::SourceUnavailable,
            SnapshotDelivery::InternetUnreachable(offline_reason) => {
                HoverFetchProgress::InternetUnreachable(offline_reason)
            }
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch,
                delivery_cadence,
            } => HoverFetchProgress::Delivered {
                snapshot_fetch,
                delivery_timing: DeliveryStamp::delivered(delivery_cadence, clock),
            },
            SnapshotDelivery::SnapshotUnchanged => match self {
                HoverFetchProgress::Delivered {
                    snapshot_fetch,
                    delivery_timing,
                } => HoverFetchProgress::Delivered {
                    snapshot_fetch,
                    delivery_timing: delivery_timing.redelivered(clock),
                },
                waiting_or_unavailable => waiting_or_unavailable,
            },
        }
    }
}

impl<S: SnapshotSource> HoverFetchedSnapshot<S>
where
    S::StreamInputs: Eq,
{
    pub fn start_hover_fetch<Message: Send + 'static>(
        &mut self,
        inputs: S::StreamInputs,
        delivered: fn(SnapshotDelivery<S>) -> Message,
        config: &Config,
        clock: Clock,
    ) -> Task<Message> {
        let schedule = self
            .remembered_by_inputs
            .get(&inputs)
            .map_or(RefetchSchedule::FetchNow, |remembered| {
                remembered.refetch_schedule(clock)
            });
        let fetch_delay = match schedule {
            RefetchSchedule::FetchNow => config.hover_cards.loading.debug_delay,
            RefetchSchedule::FetchAfter(remaining) => remaining,
        };
        let stream_inputs = inputs.clone();
        let delayed = stream::once(async move {
            sleep(fetch_delay).await;
            S::delivery_stream(stream_inputs)
        })
        .flatten();
        let (task, fetching) = Task::run(delayed, delivered).abortable();
        self.hover_fetch = SnapshotFetchStage::Fetching {
            inputs,
            fetch_handle: fetching.abort_on_drop(),
            progress: match schedule {
                RefetchSchedule::FetchNow => HoverFetchProgress::AwaitingFirstDelivery,
                RefetchSchedule::FetchAfter(_) => HoverFetchProgress::ServingRemembered,
            },
        };
        task
    }

    pub fn stop(&mut self) {
        self.hover_fetch = SnapshotFetchStage::Idle;
    }

    #[must_use]
    pub fn fetch_stage(&self) -> &SnapshotFetchStage<S> {
        &self.hover_fetch
    }

    pub fn receive(&mut self, delivery: SnapshotDelivery<S>, clock: Clock) {
        let SnapshotFetchStage::Fetching {
            inputs,
            fetch_handle,
            progress,
        } = replace(&mut self.hover_fetch, SnapshotFetchStage::Idle)
        else {
            return;
        };
        let progress = progress.delivered(delivery, clock);
        match &progress {
            HoverFetchProgress::Delivered {
                snapshot_fetch: Ok(delivered_snapshot),
                delivery_timing,
            } => {
                self.remembered_by_inputs.insert(
                    inputs.clone(),
                    RememberedSnapshot {
                        remembered_snapshot: delivered_snapshot.clone(),
                        delivery_timing: *delivery_timing,
                    },
                );
            }
            HoverFetchProgress::SourceUnavailable => {
                self.remembered_by_inputs.remove(&inputs);
            }
            HoverFetchProgress::ServingRemembered
            | HoverFetchProgress::AwaitingFirstDelivery
            | HoverFetchProgress::InternetUnreachable(_)
            | HoverFetchProgress::Delivered {
                snapshot_fetch: Err(_),
                ..
            } => {}
        }
        self.hover_fetch = SnapshotFetchStage::Fetching {
            inputs,
            fetch_handle,
            progress,
        };
    }

    pub fn sample(&self) -> Option<&S> {
        let SnapshotFetchStage::Fetching {
            inputs, progress, ..
        } = &self.hover_fetch
        else {
            return None;
        };
        match progress {
            HoverFetchProgress::Delivered {
                snapshot_fetch: Ok(delivered_snapshot),
                ..
            } => Some(delivered_snapshot),
            HoverFetchProgress::ServingRemembered
            | HoverFetchProgress::AwaitingFirstDelivery
            | HoverFetchProgress::InternetUnreachable(_) => self
                .remembered_by_inputs
                .get(inputs)
                .map(|remembered| &remembered.remembered_snapshot),
            HoverFetchProgress::SourceUnavailable
            | HoverFetchProgress::Delivered {
                snapshot_fetch: Err(_),
                ..
            } => None,
        }
    }

    pub fn sections_described_by(&self, describe: impl Fn(&S) -> Vec<Section>) -> Vec<Section> {
        let SnapshotFetchStage::Fetching {
            inputs, progress, ..
        } = &self.hover_fetch
        else {
            return Vec::new();
        };
        let remembered = self.remembered_by_inputs.get(inputs);
        let stamped = |delivery: SourceDelivery, sections: Vec<Section>| -> Vec<Section> {
            sections
                .into_iter()
                .map(|section| section.stamped_by::<S>(delivery))
                .collect()
        };
        let remembered_or_state = |delivery: SourceDelivery, detail: String| match remembered {
            Some(remembered) => stamped(delivery, describe(&remembered.remembered_snapshot)),
            None => vec![Section::source_state::<S>(delivery, detail)],
        };
        match progress {
            HoverFetchProgress::SourceUnavailable => Vec::new(),
            HoverFetchProgress::ServingRemembered => match remembered {
                Some(remembered) => stamped(
                    SourceDelivery::Delivered(remembered.delivery_timing),
                    describe(&remembered.remembered_snapshot),
                ),
                None => Vec::new(),
            },
            HoverFetchProgress::AwaitingFirstDelivery => {
                remembered_or_state(SourceDelivery::Loading, String::new())
            }
            HoverFetchProgress::InternetUnreachable(offline_reason) => {
                remembered_or_state(SourceDelivery::Offline, offline_reason.to_string())
            }
            HoverFetchProgress::Delivered {
                snapshot_fetch: Ok(delivered_snapshot),
                delivery_timing,
            } => stamped(
                SourceDelivery::Delivered(*delivery_timing),
                describe(delivered_snapshot),
            ),
            HoverFetchProgress::Delivered {
                snapshot_fetch: Err(failure),
                ..
            } => remembered_or_state(SourceDelivery::Failed, failure.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
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
    struct CountedSnapshot(u32);

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
            Ok(CountedSnapshot(count)),
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
        let Some(SectionSource::Snapshot { delivery, .. }) = sections.first().map(Section::source)
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
                .map(|section| (section.title().text().to_owned(), section.source()))
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
        assert_eq!(hover.sample(), Some(&CountedSnapshot(4)));
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
        assert_eq!(hover.sample(), Some(&CountedSnapshot(4)));
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
}
