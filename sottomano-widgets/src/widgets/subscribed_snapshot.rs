use std::mem::take;

use sottomano_desktop_sources::{SnapshotDelivery, SnapshotSource};

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
        snapshot_absence::{AbsenceCause, Staleness},
    },
    hover_cards::hover_card_body::{
        Section,
        hover_card_source_delivery::{DeliveryStamp, SourceDelivery},
    },
    widgets::icon_glyph::IconGlyph,
};

pub enum CommandOutcome<S: SnapshotSource> {
    Applied { expected_snapshot: S },
    Refused { refusal: S::Error },
    Reread { reread_fetch: Result<S, S::Error> },
}

pub enum SubscribedSnapshot<S: SnapshotSource> {
    Available {
        snapshot: S,
        delivered_at: DeliveryStamp,
        staleness: Staleness<S>,
    },
    Unavailable {
        cause: AbsenceCause<S>,
    },
}

impl<S: SnapshotSource> Default for SubscribedSnapshot<S> {
    fn default() -> Self {
        SubscribedSnapshot::Unavailable {
            cause: AbsenceCause::Awaiting,
        }
    }
}

impl<S: SnapshotSource> SubscribedSnapshot<S> {
    pub fn get(&self) -> Option<&S> {
        match self {
            SubscribedSnapshot::Available { snapshot, .. } => Some(snapshot),
            SubscribedSnapshot::Unavailable { .. } => None,
        }
    }

    #[must_use]
    pub fn awaits_first_delivery(&self) -> bool {
        matches!(
            self,
            SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::Awaiting
            }
        )
    }

    pub fn failure(&self) -> Option<&AbsenceCause<S>> {
        match self {
            SubscribedSnapshot::Unavailable {
                cause: cause @ AbsenceCause::FetchFailed(_),
            } => Some(cause),
            SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::Awaiting,
            }
            | SubscribedSnapshot::Available { .. } => None,
        }
    }

    pub fn fetch_failure(&self) -> Option<&S::Error> {
        match self {
            SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::FetchFailed(fetch_failure),
            } => Some(fetch_failure),
            SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::Awaiting,
            }
            | SubscribedSnapshot::Available { .. } => None,
        }
    }

    pub fn indicator<'a>(
        &'a self,
        describe: impl FnOnce(&'a S) -> GroupIndicator<'a>,
        absent_glyph: IconGlyph,
    ) -> GroupIndicator<'a> {
        match self {
            SubscribedSnapshot::Available {
                snapshot,
                staleness: Staleness::Current,
                ..
            } => describe(snapshot),
            SubscribedSnapshot::Available {
                snapshot,
                staleness: Staleness::StaleAfterFailure(_),
                ..
            } => describe(snapshot).in_state(IndicatorState::Failed),
            SubscribedSnapshot::Unavailable { cause } => cause.indicator(absent_glyph),
        }
    }

    pub fn receive(&mut self, delivery: SnapshotDelivery<S>, clock: Clock) {
        *self = take(self).delivered(delivery, clock);
    }

    pub fn apply_command(&mut self, command_outcome: CommandOutcome<S>) {
        let command_fetch = match command_outcome {
            CommandOutcome::Applied { expected_snapshot } => Ok(expected_snapshot),
            CommandOutcome::Refused { refusal } => Err(refusal),
            CommandOutcome::Reread { reread_fetch } => reread_fetch,
        };
        *self = take(self).with_command_fetch(command_fetch);
    }

    fn delivered(self, delivery: SnapshotDelivery<S>, clock: Clock) -> Self {
        match delivery {
            SnapshotDelivery::SourceUnavailable => SubscribedSnapshot::default(),
            SnapshotDelivery::SnapshotUnchanged => self.redelivered(clock),
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch,
                delivery_cadence,
            } => {
                let delivered_at = DeliveryStamp::delivered(delivery_cadence, clock);
                match snapshot_fetch {
                    Ok(snapshot) => SubscribedSnapshot::Available {
                        snapshot,
                        delivered_at,
                        staleness: Staleness::Current,
                    },
                    Err(fetch_error) => {
                        tracing::warn!(source = S::NAME, error = %fetch_error, "snapshot fetch failed");
                        self.failed(fetch_error, delivered_at)
                    }
                }
            }
        }
    }

    fn failed(self, fetch_error: S::Error, delivered_at: DeliveryStamp) -> Self {
        match self {
            SubscribedSnapshot::Available { snapshot, .. } => SubscribedSnapshot::Available {
                snapshot,
                delivered_at,
                staleness: Staleness::StaleAfterFailure(fetch_error),
            },
            SubscribedSnapshot::Unavailable { .. } => SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::FetchFailed(fetch_error),
            },
        }
    }

    fn with_command_fetch(self, command_fetch: Result<S, S::Error>) -> Self {
        let SubscribedSnapshot::Available { delivered_at, .. } = self else {
            return self;
        };
        match command_fetch {
            Ok(snapshot) => SubscribedSnapshot::Available {
                snapshot,
                delivered_at,
                staleness: Staleness::Current,
            },
            Err(refusal) => SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::FetchFailed(refusal),
            },
        }
    }

    fn redelivered(self, clock: Clock) -> Self {
        match self {
            SubscribedSnapshot::Available {
                snapshot,
                delivered_at,
                staleness,
            } => SubscribedSnapshot::Available {
                snapshot,
                delivered_at: delivered_at.redelivered(clock),
                staleness,
            },
            SubscribedSnapshot::Unavailable { .. } => self,
        }
    }

    pub fn sections_described_by(&self, describe: impl FnOnce(&S) -> Vec<Section>) -> Vec<Section> {
        match self {
            SubscribedSnapshot::Available {
                snapshot,
                delivered_at,
                staleness: Staleness::Current,
            } => describe(snapshot)
                .into_iter()
                .map(|section| section.stamped_by::<S>(SourceDelivery::Delivered(*delivered_at)))
                .collect(),
            SubscribedSnapshot::Available {
                snapshot,
                staleness: Staleness::StaleAfterFailure(fetch_failure),
                ..
            } => describe(snapshot)
                .into_iter()
                .map(|section| section.stamped_by::<S>(SourceDelivery::Failed))
                .chain([Section::source_state::<S>(
                    SourceDelivery::Failed,
                    fetch_failure.to_string(),
                )])
                .collect(),
            SubscribedSnapshot::Unavailable { cause } => vec![cause.source_state()],
        }
    }

    pub fn hover_card_with(&self, sections: Vec<Section>) -> Vec<Section> {
        self.sections_described_by(|_| sections)
    }

    pub fn visible_indicator(
        &self,
        panel_visibility: impl FnOnce(&S) -> PanelVisibility,
    ) -> Option<&S> {
        self.get()
            .filter(|snapshot| panel_visibility(snapshot) == PanelVisibility::Shown)
    }

    pub fn shown_state(
        &self,
        held_state: impl FnOnce(&S) -> IndicatorState,
        panel_visibility: impl FnOnce(&S) -> PanelVisibility,
    ) -> Option<IndicatorState> {
        match self {
            SubscribedSnapshot::Available {
                snapshot,
                staleness,
                ..
            } => (panel_visibility(snapshot) == PanelVisibility::Shown).then(|| match staleness {
                Staleness::Current => held_state(snapshot),
                Staleness::StaleAfterFailure(_) => IndicatorState::Failed,
            }),
            SubscribedSnapshot::Unavailable {
                cause: AbsenceCause::Awaiting,
            } => None,
            SubscribedSnapshot::Unavailable { cause } => Some(cause.shown_state()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fmt::{Display, Formatter, Result as FmtResult},
        time::{Duration, SystemTime},
    };

    use iced::futures::{Stream, stream};
    use sottomano_desktop_sources::delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling};

    use super::*;
    use crate::{
        group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
        widgets::icon_glyph::IconGlyph,
    };

    #[derive(Debug, Clone, PartialEq)]
    struct CountedSnapshot {
        snapshot_count: u32,
    }

    #[derive(Debug, Clone)]
    struct CountingFailure;

    impl Display for CountingFailure {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "counting failed")
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
                cadence: DeliveryCadence::PeriodicPolling(PeriodicPolling::every(
                    Duration::from_secs(3)
                )),
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
}
