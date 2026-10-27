use std::{collections::HashMap, mem::replace, num::NonZeroUsize, time::Duration};

use iced::{
    Task,
    futures::{StreamExt, stream},
    task::Handle,
};
use sottomano_config::Config;
use sottomano_sources::{SnapshotDelivery, SnapshotSource};
use tokio::time::sleep;

use crate::{
    clock::Clock,
    widgets::tooltip_body::{
        DeliveryTiming, ReadingSections, Section, SourcedSections, TooltipBody,
    },
};

const FIRST_FETCH_PENDING_LINES: NonZeroUsize = NonZeroUsize::new(3).unwrap();

#[derive(Debug, Clone)]
pub struct RememberedSnapshot<S: SnapshotSource> {
    remembered_snapshot: S,
    delivery_timing: DeliveryTiming,
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
    Delivered {
        snapshot_fetch: Result<S, S::Error>,
        delivery_timing: DeliveryTiming,
    },
}

#[derive(Debug)]
pub enum HoverFetch<S: SnapshotSource> {
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
    hover_fetch: HoverFetch<S>,
}

impl<S: SnapshotSource> Default for HoverFetchedSnapshot<S> {
    fn default() -> Self {
        Self {
            remembered_by_inputs: HashMap::new(),
            hover_fetch: HoverFetch::Idle,
        }
    }
}

impl<S: SnapshotSource> RememberedSnapshot<S> {
    fn refetch_schedule(&self, clock: Clock) -> RefetchSchedule {
        let (DeliveryTiming::PeriodicPolling {
            poll_period: refresh_period,
            delivered_at,
        }
        | DeliveryTiming::LivePushWithRecheck {
            recheck_period: refresh_period,
            delivered_at,
        }) = self.delivery_timing
        else {
            return RefetchSchedule::FetchNow;
        };
        let elapsed = clock
            .wall
            .duration_since(delivered_at)
            .unwrap_or(Duration::ZERO);
        match refresh_period.checked_sub(elapsed) {
            Some(remaining) if !remaining.is_zero() => RefetchSchedule::FetchAfter(remaining),
            Some(_) | None => RefetchSchedule::FetchNow,
        }
    }
}

impl<S: SnapshotSource> HoverFetchProgress<S> {
    fn delivered(self, delivery: SnapshotDelivery<S>, clock: Clock) -> Self {
        match delivery {
            SnapshotDelivery::SourceUnavailable => HoverFetchProgress::SourceUnavailable,
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch,
                stamped_cadence,
            } => HoverFetchProgress::Delivered {
                snapshot_fetch,
                delivery_timing: DeliveryTiming::from_delivery_cadence(
                    stamped_cadence.delivery_cadence(),
                    clock,
                ),
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
            RefetchSchedule::FetchNow => config.tooltips.loading.debug_delay,
            RefetchSchedule::FetchAfter(remaining) => remaining,
        };
        let stream_inputs = inputs.clone();
        let delayed = stream::once(async move {
            sleep(fetch_delay).await;
            S::delivery_stream(stream_inputs)
        })
        .flatten();
        let (task, fetching) = Task::run(delayed, delivered).abortable();
        self.hover_fetch = HoverFetch::Fetching {
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
        self.hover_fetch = HoverFetch::Idle;
    }

    #[must_use]
    pub fn is_hovered(&self) -> bool {
        matches!(self.hover_fetch, HoverFetch::Fetching { .. })
    }

    pub fn receive(&mut self, delivery: SnapshotDelivery<S>, clock: Clock) {
        let HoverFetch::Fetching {
            inputs,
            fetch_handle,
            progress,
        } = replace(&mut self.hover_fetch, HoverFetch::Idle)
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
            | HoverFetchProgress::Delivered {
                snapshot_fetch: Err(_),
                ..
            } => {}
        }
        self.hover_fetch = HoverFetch::Fetching {
            inputs,
            fetch_handle,
            progress,
        };
    }

    pub fn reading(&self) -> Option<&S> {
        let HoverFetch::Fetching {
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
            HoverFetchProgress::ServingRemembered | HoverFetchProgress::AwaitingFirstDelivery => {
                self.remembered_by_inputs
                    .get(inputs)
                    .map(|remembered| &remembered.remembered_snapshot)
            }
            HoverFetchProgress::SourceUnavailable
            | HoverFetchProgress::Delivered {
                snapshot_fetch: Err(_),
                ..
            } => None,
        }
    }

    pub fn sections_described_by(
        &self,
        describe: impl Fn(&S) -> Vec<Section>,
    ) -> Option<SourcedSections>
    where
        S::Error: Into<Section>,
    {
        let HoverFetch::Fetching {
            inputs, progress, ..
        } = &self.hover_fetch
        else {
            return None;
        };
        let remembered = self.remembered_by_inputs.get(inputs);
        match progress {
            HoverFetchProgress::ServingRemembered => {
                let remembered = remembered?;
                Some(SourcedSections::delivered(
                    TooltipBody::sections(describe(&remembered.remembered_snapshot)),
                    remembered.delivery_timing,
                ))
            }
            HoverFetchProgress::AwaitingFirstDelivery => Some(SourcedSections::awaiting_delivery(
                TooltipBody::sections(remembered.map_or_else(
                    || vec![Section::pending_lines(FIRST_FETCH_PENDING_LINES)],
                    |remembered| describe(&remembered.remembered_snapshot),
                )),
            )),
            HoverFetchProgress::SourceUnavailable => None,
            HoverFetchProgress::Delivered {
                snapshot_fetch,
                delivery_timing,
            } => Some(SourcedSections::delivered(
                TooltipBody::sections(match snapshot_fetch {
                    Ok(delivered_snapshot) => describe(delivered_snapshot),
                    Err(failure) => vec![failure.clone().into()],
                }),
                *delivery_timing,
            )),
        }
    }
}

impl<S: SnapshotSource<Error: Into<Section>> + ReadingSections> HoverFetchedSnapshot<S>
where
    S::StreamInputs: Eq,
{
    pub fn sections(&self, clock: Clock) -> Option<SourcedSections> {
        self.sections_described_by(|delivered_snapshot| delivered_snapshot.sections(clock))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fmt::{Display, Formatter, Result as FmtResult},
        time::SystemTime,
    };

    use iced::futures::Stream;
    use sottomano_sources::delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling};

    use super::*;
    use crate::widgets::tooltip_body::SectionsDeliveryState;

    #[derive(Debug, Clone, PartialEq)]
    struct CountedSnapshot(u32);

    #[derive(Debug, Clone)]
    struct CountingFailure;

    impl Display for CountingFailure {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
            write!(formatter, "counting failed")
        }
    }

    impl From<CountingFailure> for Section {
        fn from(failure: CountingFailure) -> Self {
            Section::error(failure)
        }
    }

    impl SnapshotSource for CountedSnapshot {
        type Error = CountingFailure;
        type StreamInputs = u32;

        const NAME: &'static str = "counted-snapshot";

        fn delivery_stream(_inputs: u32) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
            stream::empty()
        }
    }

    const POLL_PERIOD: Duration = Duration::from_mins(10);

    fn clock_at(seconds_since_epoch: u64) -> Clock {
        Clock {
            wall: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_since_epoch),
            ..Clock::real()
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

    fn delivery_state(hover: &HoverFetchedSnapshot<CountedSnapshot>) -> SectionsDeliveryState {
        hover
            .sections_described_by(|_| vec![Section::note("counted", None)])
            .unwrap()
            .delivery_state()
    }

    #[test]
    fn a_first_hover_shows_a_loading_state_without_a_cadence() {
        let mut hover = HoverFetchedSnapshot::default();
        hovered_at(&mut hover, 1, clock_at(0));
        assert_eq!(
            delivery_state(&hover),
            SectionsDeliveryState::AwaitingDelivery
        );
        assert!(hover.reading().is_none());
    }

    #[test]
    fn a_rehover_within_the_poll_period_serves_the_remembered_snapshot() {
        let mut hover = HoverFetchedSnapshot::default();
        hovered_at(&mut hover, 1, clock_at(0));
        hover.receive(polled_snapshot(4), clock_at(0));
        hover.stop();
        hovered_at(&mut hover, 1, clock_at(60));
        assert_eq!(hover.reading(), Some(&CountedSnapshot(4)));
        assert_eq!(
            delivery_state(&hover),
            SectionsDeliveryState::Delivered {
                delivery_timing: DeliveryTiming::PeriodicPolling {
                    poll_period: POLL_PERIOD,
                    delivered_at: SystemTime::UNIX_EPOCH,
                },
            }
        );
    }

    #[test]
    fn a_rehover_after_the_poll_period_loads_while_showing_the_remembered_snapshot() {
        let mut hover = HoverFetchedSnapshot::default();
        hovered_at(&mut hover, 1, clock_at(0));
        hover.receive(polled_snapshot(4), clock_at(0));
        hover.stop();
        hovered_at(&mut hover, 1, clock_at(601));
        assert_eq!(hover.reading(), Some(&CountedSnapshot(4)));
        assert_eq!(
            delivery_state(&hover),
            SectionsDeliveryState::AwaitingDelivery
        );
    }

    #[test]
    fn another_window_never_shows_the_previous_windows_snapshot() {
        let mut hover = HoverFetchedSnapshot::default();
        hovered_at(&mut hover, 1, clock_at(0));
        hover.receive(polled_snapshot(4), clock_at(0));
        hover.stop();
        hovered_at(&mut hover, 2, clock_at(30));
        assert!(hover.reading().is_none());
        assert_eq!(
            delivery_state(&hover),
            SectionsDeliveryState::AwaitingDelivery
        );
    }

    #[test]
    fn an_unavailable_source_forgets_its_remembered_snapshot() {
        let mut hover = HoverFetchedSnapshot::default();
        hovered_at(&mut hover, 1, clock_at(0));
        hover.receive(polled_snapshot(4), clock_at(0));
        hover.receive(SnapshotDelivery::SourceUnavailable, clock_at(3));
        hover.stop();
        hovered_at(&mut hover, 1, clock_at(30));
        assert!(hover.reading().is_none());
    }
}
