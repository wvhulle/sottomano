use std::mem::take;

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
        DeliveryTiming, Readiness, ReadingSections, Section, SourcedSections, TooltipBody,
    },
};

#[derive(Debug)]
pub enum RememberedSnapshot<S: SnapshotSource> {
    NeverFetched,
    Remembered {
        remembered_snapshot: S,
        delivery_timing: DeliveryTiming,
    },
}

#[derive(Debug)]
pub enum HoverFetchProgress<S: SnapshotSource> {
    AwaitingFirstDelivery,
    SourceUnavailable,
    Delivered {
        snapshot_fetch: Result<S, S::Error>,
        delivery_timing: DeliveryTiming,
    },
}

#[derive(Debug)]
pub enum HoverFetchedSnapshot<S: SnapshotSource> {
    Idle {
        remembered: RememberedSnapshot<S>,
    },
    Fetching {
        fetch_handle: Handle,
        remembered: RememberedSnapshot<S>,
        progress: HoverFetchProgress<S>,
    },
}

impl<S: SnapshotSource> Default for HoverFetchedSnapshot<S> {
    fn default() -> Self {
        HoverFetchedSnapshot::Idle {
            remembered: RememberedSnapshot::NeverFetched,
        }
    }
}

impl<S: SnapshotSource> HoverFetchProgress<S> {
    fn remembered_after(self, earlier: RememberedSnapshot<S>) -> RememberedSnapshot<S> {
        match self {
            HoverFetchProgress::Delivered {
                snapshot_fetch: Ok(remembered_snapshot),
                delivery_timing,
            } => RememberedSnapshot::Remembered {
                remembered_snapshot,
                delivery_timing,
            },
            HoverFetchProgress::SourceUnavailable => RememberedSnapshot::NeverFetched,
            HoverFetchProgress::AwaitingFirstDelivery
            | HoverFetchProgress::Delivered {
                snapshot_fetch: Err(_),
                ..
            } => earlier,
        }
    }

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
                awaiting_or_unavailable => awaiting_or_unavailable,
            },
        }
    }
}

impl<S: SnapshotSource> HoverFetchedSnapshot<S> {
    pub fn start_hover_fetch<Message: Send + 'static>(
        &mut self,
        inputs: S::StreamInputs,
        delivered: fn(SnapshotDelivery<S>) -> Message,
        config: &Config,
    ) -> Task<Message> {
        let debug_delay = config.tooltips.loading.debug_delay;
        let delayed = stream::once(async move {
            sleep(debug_delay).await;
            S::delivery_stream(inputs)
        })
        .flatten();
        let (task, fetching) = Task::run(delayed, delivered).abortable();
        *self = HoverFetchedSnapshot::Fetching {
            fetch_handle: fetching.abort_on_drop(),
            remembered: take(self).into_remembered(),
            progress: HoverFetchProgress::AwaitingFirstDelivery,
        };
        task
    }

    pub fn stop(&mut self) {
        *self = HoverFetchedSnapshot::Idle {
            remembered: take(self).into_remembered(),
        };
    }

    #[must_use]
    pub fn is_hovered(&self) -> bool {
        matches!(self, HoverFetchedSnapshot::Fetching { .. })
    }

    fn into_remembered(self) -> RememberedSnapshot<S> {
        match self {
            HoverFetchedSnapshot::Idle { remembered } => remembered,
            HoverFetchedSnapshot::Fetching {
                remembered,
                progress,
                ..
            } => progress.remembered_after(remembered),
        }
    }

    pub fn receive(&mut self, delivery: SnapshotDelivery<S>, clock: Clock) {
        let HoverFetchedSnapshot::Fetching {
            fetch_handle,
            remembered,
            progress,
        } = take(self)
        else {
            return;
        };
        *self = HoverFetchedSnapshot::Fetching {
            fetch_handle,
            remembered: match delivery {
                SnapshotDelivery::SourceUnavailable => RememberedSnapshot::NeverFetched,
                SnapshotDelivery::SnapshotDelivered { .. }
                | SnapshotDelivery::SnapshotUnchanged => remembered,
            },
            progress: progress.delivered(delivery, clock),
        };
    }

    pub fn reading(&self) -> Option<&S> {
        let remembered = match self {
            HoverFetchedSnapshot::Idle { remembered }
            | HoverFetchedSnapshot::Fetching {
                remembered,
                progress: HoverFetchProgress::AwaitingFirstDelivery,
                ..
            } => remembered,
            HoverFetchedSnapshot::Fetching {
                progress:
                    HoverFetchProgress::Delivered {
                        snapshot_fetch: Ok(delivered_snapshot),
                        ..
                    },
                ..
            } => return Some(delivered_snapshot),
            HoverFetchedSnapshot::Fetching {
                progress:
                    HoverFetchProgress::SourceUnavailable
                    | HoverFetchProgress::Delivered {
                        snapshot_fetch: Err(_),
                        ..
                    },
                ..
            } => return None,
        };
        match remembered {
            RememberedSnapshot::Remembered {
                remembered_snapshot,
                ..
            } => Some(remembered_snapshot),
            RememberedSnapshot::NeverFetched => None,
        }
    }

    pub fn sections_described_by(
        &self,
        describe: impl Fn(&S) -> Vec<Section>,
    ) -> Option<SourcedSections>
    where
        S::Error: Into<Section>,
    {
        let estimated_from = |remembered: &RememberedSnapshot<S>| match remembered {
            RememberedSnapshot::Remembered {
                remembered_snapshot,
                delivery_timing,
            } => Some(SourcedSections::new(
                Readiness::Estimated,
                *delivery_timing,
                TooltipBody::sections(describe(remembered_snapshot)),
            )),
            RememberedSnapshot::NeverFetched => None,
        };
        match self {
            HoverFetchedSnapshot::Idle { remembered }
            | HoverFetchedSnapshot::Fetching {
                remembered,
                progress: HoverFetchProgress::AwaitingFirstDelivery,
                ..
            } => estimated_from(remembered),
            HoverFetchedSnapshot::Fetching {
                progress: HoverFetchProgress::SourceUnavailable,
                ..
            } => None,
            HoverFetchedSnapshot::Fetching {
                progress:
                    HoverFetchProgress::Delivered {
                        snapshot_fetch,
                        delivery_timing,
                    },
                ..
            } => Some(SourcedSections::new(
                Readiness::Settled,
                *delivery_timing,
                TooltipBody::sections(match snapshot_fetch {
                    Ok(delivered_snapshot) => describe(delivered_snapshot),
                    Err(failure) => vec![failure.clone().into()],
                }),
            )),
        }
    }
}

impl<S: SnapshotSource<Error: Into<Section>> + ReadingSections> HoverFetchedSnapshot<S> {
    pub fn sections(&self, clock: Clock) -> Option<SourcedSections> {
        self.sections_described_by(|delivered_snapshot| delivered_snapshot.sections(clock))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fmt::{Display, Formatter, Result as FmtResult},
        time::{Duration, SystemTime},
    };

    use iced::futures::Stream;
    use sottomano_sources::delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling};

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct CountedSnapshot(u32);

    #[derive(Debug, Clone)]
    struct CountingFailure;

    impl Display for CountingFailure {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "counting failed")
        }
    }

    impl From<CountingFailure> for Section {
        fn from(failure: CountingFailure) -> Self {
            Section::error(failure)
        }
    }

    impl SnapshotSource for CountedSnapshot {
        type Error = CountingFailure;
        type StreamInputs = ();

        const NAME: &'static str = "counted-snapshot";

        fn delivery_stream(_inputs: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
            stream::empty()
        }
    }

    fn fetching_hover() -> HoverFetchedSnapshot<CountedSnapshot> {
        let (_task, fetch_handle) = Task::<()>::none().abortable();
        HoverFetchedSnapshot::Fetching {
            fetch_handle: fetch_handle.abort_on_drop(),
            remembered: RememberedSnapshot::NeverFetched,
            progress: HoverFetchProgress::AwaitingFirstDelivery,
        }
    }

    fn polled_snapshot(count: u32) -> SnapshotDelivery<CountedSnapshot> {
        SnapshotDelivery::fixture_delivered(
            Ok(CountedSnapshot(count)),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(3))),
        )
    }

    fn clock_at(seconds_since_epoch: u64) -> Clock {
        Clock {
            wall: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_since_epoch),
            ..Clock::real()
        }
    }

    #[test]
    fn a_hover_that_never_fetched_shows_nothing_until_its_first_delivery() {
        let hover = fetching_hover();
        assert!(hover.sections_described_by(|_| Vec::new()).is_none());
        assert!(hover.reading().is_none());
    }

    #[test]
    fn a_stopped_hover_remembers_its_snapshot_with_its_own_timing() {
        let mut hover = fetching_hover();
        hover.receive(polled_snapshot(4), clock_at(10));
        hover.stop();
        assert!(!hover.is_hovered());
        assert_eq!(hover.reading(), Some(&CountedSnapshot(4)));
        let estimated = hover.sections_described_by(|_| Vec::new()).unwrap();
        assert_eq!(
            estimated.delivery_timing(),
            DeliveryTiming::PeriodicPolling {
                poll_period: Duration::from_secs(3),
                delivered_at: SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            }
        );
    }

    #[test]
    fn an_unavailable_source_forgets_the_remembered_snapshot() {
        let mut hover = fetching_hover();
        hover.receive(polled_snapshot(4), clock_at(10));
        hover.receive(SnapshotDelivery::SourceUnavailable, clock_at(13));
        hover.stop();
        assert!(hover.reading().is_none());
    }
}
