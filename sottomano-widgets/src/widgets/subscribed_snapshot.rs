use std::{
    borrow::Cow,
    convert::Infallible,
    fmt::{Display, Formatter, Result as FmtResult},
    marker::PhantomData,
    mem::take,
};

use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::internet_connectivity_gate::InternetUnreachable,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::{GroupItemReading, PanelVisibility},
        offline_internet_reading::{OfflineSnapshot, PlaceholderCause, SnapshotPlaceholder},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            ReadingSections, Section,
            hover_card_source_delivery::{DeliveryStamp, SourceDelivery},
        },
    },
    widgets::icon_glyph::IconGlyph,
};

pub struct FailedSnapshotFetch<S: SnapshotSource> {
    name: Cow<'static, str>,
    icon: IconGlyph,
    error: S::Error,
}

impl<S: SnapshotSource + GroupItemReading> FailedSnapshotFetch<S> {
    pub fn from_fetch_error(error: S::Error) -> Self {
        Self {
            name: S::NAME.into(),
            icon: S::fallback_icon(),
            error,
        }
    }
}

impl<S: SnapshotSource> GroupItemReading for FailedSnapshotFetch<S> {
    fn name(&self) -> Cow<'static, str> {
        self.name.clone()
    }

    fn icon(&self) -> IconGlyph {
        self.icon
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        Vec::new()
    }
}

impl<S: SnapshotSource> Display for FailedSnapshotFetch<S> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        write!(formatter, "{}", self.error)
    }
}

impl GroupItemReading for Infallible {
    fn name(&self) -> Cow<'static, str> {
        match *self {}
    }

    fn icon(&self) -> IconGlyph {
        match *self {}
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        match *self {}
    }
}

pub enum CommandOutcome<S: SnapshotSource> {
    Applied { expected_snapshot: S },
    Refused { refusal: S::Error },
    Reread { reread_fetch: Result<S, S::Error> },
}

pub struct ShowsFetchFailures;
pub struct HidesFailuresAfterSuccess;
pub struct HidesFetchFailures;

pub trait FetchFailurePolicy<S: SnapshotSource>: Sized {
    type ShownFailure: GroupItemReading + Display;

    fn after_failed_fetch(
        latest: SubscribedSnapshot<S, Self>,
        error: S::Error,
        delivery_timing: DeliveryStamp,
    ) -> SubscribedSnapshot<S, Self>;
}

#[derive(Default)]
pub enum SubscribedSnapshot<S, Policy = ShowsFetchFailures>
where
    S: SnapshotSource,
    Policy: FetchFailurePolicy<S>,
{
    #[default]
    AwaitingFirstSnapshot,
    SourceUnavailable,
    SnapshotHeld {
        held_snapshot: S,
        delivery_timing: DeliveryStamp,
    },
    SnapshotFetchFailed {
        fetch_failure: Policy::ShownFailure,
        delivery_timing: DeliveryStamp,
    },
    SnapshotHeldOffline {
        offline_snapshot: OfflineSnapshot<S>,
    },
    OfflineBeforeSnapshot {
        offline_placeholder: SnapshotPlaceholder<S>,
    },
}

impl<S: SnapshotSource + GroupItemReading> FetchFailurePolicy<S> for ShowsFetchFailures {
    type ShownFailure = FailedSnapshotFetch<S>;

    fn after_failed_fetch(
        latest: SubscribedSnapshot<S, Self>,
        error: S::Error,
        delivery_timing: DeliveryStamp,
    ) -> SubscribedSnapshot<S, Self> {
        let fetch_failure = match latest {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. }
            | SubscribedSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                ..
            } => FailedSnapshotFetch {
                name: held_snapshot.name(),
                icon: held_snapshot.icon(),
                error,
            },
            SubscribedSnapshot::SnapshotFetchFailed { fetch_failure, .. } => FailedSnapshotFetch {
                error,
                ..fetch_failure
            },
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::OfflineBeforeSnapshot { .. } => {
                FailedSnapshotFetch::from_fetch_error(error)
            }
        };
        SubscribedSnapshot::SnapshotFetchFailed {
            fetch_failure,
            delivery_timing,
        }
    }
}

impl<S: SnapshotSource + GroupItemReading> FetchFailurePolicy<S> for HidesFailuresAfterSuccess {
    type ShownFailure = FailedSnapshotFetch<S>;

    fn after_failed_fetch(
        latest: SubscribedSnapshot<S, Self>,
        error: S::Error,
        delivery_timing: DeliveryStamp,
    ) -> SubscribedSnapshot<S, Self> {
        match latest {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. }
            | SubscribedSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                ..
            } => {
                tracing::warn!("{}: {error}", S::NAME);
                SubscribedSnapshot::SnapshotHeld {
                    held_snapshot,
                    delivery_timing,
                }
            }
            SubscribedSnapshot::SnapshotFetchFailed { fetch_failure, .. } => {
                SubscribedSnapshot::SnapshotFetchFailed {
                    fetch_failure: FailedSnapshotFetch {
                        error,
                        ..fetch_failure
                    },
                    delivery_timing,
                }
            }
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::OfflineBeforeSnapshot { .. } => {
                SubscribedSnapshot::SnapshotFetchFailed {
                    fetch_failure: FailedSnapshotFetch::from_fetch_error(error),
                    delivery_timing,
                }
            }
        }
    }
}

impl<S: SnapshotSource> FetchFailurePolicy<S> for HidesFetchFailures {
    type ShownFailure = Infallible;

    fn after_failed_fetch(
        latest: SubscribedSnapshot<S, Self>,
        error: S::Error,
        delivery_timing: DeliveryStamp,
    ) -> SubscribedSnapshot<S, Self> {
        tracing::warn!("{}: {error}", S::NAME);
        match latest {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. }
            | SubscribedSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                ..
            } => SubscribedSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            },
            SubscribedSnapshot::SnapshotFetchFailed { fetch_failure, .. } => match fetch_failure {},
            awaiting_or_unavailable => awaiting_or_unavailable,
        }
    }
}

impl<S: SnapshotSource, Policy: FetchFailurePolicy<S>> SubscribedSnapshot<S, Policy> {
    pub fn get(&self) -> Option<&S> {
        match self {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. }
            | SubscribedSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                ..
            } => Some(held_snapshot),
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::SnapshotFetchFailed { .. }
            | SubscribedSnapshot::OfflineBeforeSnapshot { .. } => None,
        }
    }

    fn delivery_timing(&self) -> Option<DeliveryStamp> {
        match self {
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::OfflineBeforeSnapshot { .. } => None,
            SubscribedSnapshot::SnapshotHeld {
                delivery_timing, ..
            }
            | SubscribedSnapshot::SnapshotFetchFailed {
                delivery_timing, ..
            } => Some(*delivery_timing),
            SubscribedSnapshot::SnapshotHeldOffline { offline_snapshot } => {
                Some(offline_snapshot.delivery_timing)
            }
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
            SnapshotDelivery::SourceUnavailable => SubscribedSnapshot::SourceUnavailable,
            SnapshotDelivery::InternetUnreachable(offline_reason) => self.offline(offline_reason),
            SnapshotDelivery::SnapshotUnchanged => self.redelivered(clock),
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch,
                delivery_cadence,
            } => {
                let delivery_timing = DeliveryStamp::delivered(delivery_cadence, clock);
                match snapshot_fetch {
                    Ok(held_snapshot) => SubscribedSnapshot::SnapshotHeld {
                        held_snapshot,
                        delivery_timing,
                    },
                    Err(error) => Policy::after_failed_fetch(self, error, delivery_timing),
                }
            }
        }
    }

    fn offline(self, offline_reason: InternetUnreachable) -> Self {
        match self {
            SubscribedSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            }
            | SubscribedSnapshot::SnapshotHeldOffline {
                offline_snapshot:
                    OfflineSnapshot {
                        held_snapshot,
                        delivery_timing,
                        ..
                    },
            } => SubscribedSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot {
                    held_snapshot,
                    offline_reason: Box::new(offline_reason),
                    delivery_timing,
                },
            },
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::SnapshotFetchFailed { .. }
            | SubscribedSnapshot::OfflineBeforeSnapshot { .. } => {
                SubscribedSnapshot::OfflineBeforeSnapshot {
                    offline_placeholder: SnapshotPlaceholder {
                        placeholder_cause: PlaceholderCause::Offline(offline_reason),
                        placeholder_source: PhantomData,
                    },
                }
            }
        }
    }

    fn with_command_fetch(self, command_fetch: Result<S, S::Error>) -> Self {
        let Some(delivery_timing) = self.delivery_timing() else {
            return self;
        };
        match command_fetch {
            Ok(held_snapshot) => SubscribedSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            },
            Err(error) => Policy::after_failed_fetch(self, error, delivery_timing),
        }
    }

    fn redelivered(self, clock: Clock) -> Self {
        match self {
            SubscribedSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            } => SubscribedSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing: delivery_timing.redelivered(clock),
            },
            SubscribedSnapshot::SnapshotFetchFailed {
                fetch_failure,
                delivery_timing,
            } => SubscribedSnapshot::SnapshotFetchFailed {
                fetch_failure,
                delivery_timing: delivery_timing.redelivered(clock),
            },
            SubscribedSnapshot::SnapshotHeldOffline { offline_snapshot } => {
                SubscribedSnapshot::SnapshotHeldOffline {
                    offline_snapshot: OfflineSnapshot {
                        delivery_timing: offline_snapshot.delivery_timing.redelivered(clock),
                        ..offline_snapshot
                    },
                }
            }
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::OfflineBeforeSnapshot { .. } => self,
        }
    }

    pub fn sections_described_by(&self, describe: impl FnOnce(&S) -> Vec<Section>) -> Vec<Section> {
        let stamped = |delivery: SourceDelivery, sections: Vec<Section>| -> Vec<Section> {
            sections
                .into_iter()
                .map(|section| section.stamped_by::<S>(delivery))
                .collect()
        };
        match self {
            SubscribedSnapshot::SourceUnavailable => Vec::new(),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                vec![Section::source_state::<S>(SourceDelivery::Loading, "")]
            }
            SubscribedSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            } => stamped(
                SourceDelivery::Delivered(*delivery_timing),
                describe(held_snapshot),
            ),
            SubscribedSnapshot::SnapshotFetchFailed { fetch_failure, .. } => {
                vec![Section::source_state::<S>(
                    SourceDelivery::Failed,
                    fetch_failure.to_string(),
                )]
            }
            SubscribedSnapshot::SnapshotHeldOffline { offline_snapshot } => stamped(
                SourceDelivery::Offline,
                describe(&offline_snapshot.held_snapshot),
            ),
            SubscribedSnapshot::OfflineBeforeSnapshot {
                offline_placeholder,
            } => vec![offline_placeholder.source_state()],
        }
    }

    pub fn hover_card_with(&self, sections: Vec<Section>) -> Vec<Section> {
        self.sections_described_by(|_| sections)
    }

    pub fn failure(&self) -> Option<&dyn GroupItemReading> {
        match self {
            SubscribedSnapshot::SnapshotFetchFailed { fetch_failure, .. } => Some(fetch_failure),
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::SnapshotHeld { .. }
            | SubscribedSnapshot::SnapshotHeldOffline { .. }
            | SubscribedSnapshot::OfflineBeforeSnapshot { .. } => None,
        }
    }
}

impl<S: SnapshotSource + ReadingSections, Policy: FetchFailurePolicy<S>>
    SubscribedSnapshot<S, Policy>
{
    pub fn sections(&self, clock: Clock) -> Vec<Section> {
        self.sections_described_by(|held_snapshot| held_snapshot.sections(clock))
    }
}

fn awaiting_snapshot_placeholder<S>() -> SnapshotPlaceholder<S> {
    SnapshotPlaceholder {
        placeholder_cause: PlaceholderCause::AwaitingFirstSnapshot,
        placeholder_source: PhantomData,
    }
}

impl<S: SnapshotSource + GroupItemReading, Policy: FetchFailurePolicy<S>>
    SubscribedSnapshot<S, Policy>
{
    pub fn visible_reading(&self) -> Option<&S> {
        self.get()
            .filter(|held_snapshot| held_snapshot.panel_visibility() == PanelVisibility::Shown)
    }

    pub fn display(&self) -> Option<&dyn GroupItemReading> {
        match self {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                (held_snapshot.panel_visibility() == PanelVisibility::Shown)
                    .then_some(held_snapshot as &dyn GroupItemReading)
            }
            SubscribedSnapshot::SnapshotFetchFailed { fetch_failure, .. } => Some(fetch_failure),
            SubscribedSnapshot::SnapshotHeldOffline {
                offline_snapshot, ..
            } => (offline_snapshot.panel_visibility() == PanelVisibility::Shown)
                .then_some(offline_snapshot as &dyn GroupItemReading),
            SubscribedSnapshot::OfflineBeforeSnapshot {
                offline_placeholder,
            } => Some(offline_placeholder),
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {
                None
            }
        }
    }

    pub fn group_item<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
    ) -> Option<GroupItem<'_, M>> {
        self.display()
            .map(|widget| GroupItem::reading(widget, target))
    }

    pub fn group_item_while_pending<M: 'static>(
        &self,
        target: impl Into<HoverTarget>,
    ) -> Option<GroupItem<'_, M>>
    where
        S: 'static,
    {
        if matches!(self, SubscribedSnapshot::AwaitingFirstSnapshot) {
            return Some(GroupItem::reading(
                awaiting_snapshot_placeholder::<S>(),
                target,
            ));
        }
        self.group_item(target)
    }

    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.sections_described_by(|held_snapshot| held_snapshot.hover_card(clock))
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

    #[derive(Debug, Clone, PartialEq)]
    struct CountedSnapshot(u32);

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

    impl GroupItemReading for CountedSnapshot {
        fn name(&self) -> Cow<'static, str> {
            "Counted".into()
        }

        fn icon(&self) -> IconGlyph {
            Self::fallback_icon()
        }

        fn hover_card(&self, _clock: Clock) -> Vec<Section> {
            Vec::new()
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
        assert!(matches!(latest, SubscribedSnapshot::AwaitingFirstSnapshot));
        assert!(latest.delivery_timing().is_none());
    }

    #[test]
    fn an_unchanged_snapshot_keeps_its_poll_period_and_moves_its_delivery_time() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), timeline.at(10));
        latest.receive(SnapshotDelivery::SnapshotUnchanged, timeline.at(13));
        assert_eq!(
            latest.delivery_timing(),
            Some(DeliveryStamp {
                cadence: DeliveryCadence::PeriodicPolling(PeriodicPolling::every(
                    Duration::from_secs(3)
                )),
                delivered_at: timeline.at(13).instant,
            })
        );
    }

    #[test]
    fn a_holder_that_hides_failures_keeps_its_snapshot() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot, HidesFetchFailures> =
            SubscribedSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), timeline.at(10));
        latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(13));
        assert_eq!(latest.get(), Some(&CountedSnapshot(1)));
    }

    #[test]
    fn failures_after_a_success_are_hidden_but_failures_before_it_are_shown() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot, HidesFailuresAfterSuccess> =
            SubscribedSnapshot::default();
        latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(10));
        assert!(latest.failure().is_some());
        latest.receive(polled_delivery(Ok(CountedSnapshot(2))), timeline.at(13));
        latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(16));
        assert_eq!(latest.get(), Some(&CountedSnapshot(2)));
    }

    #[test]
    fn a_command_before_any_delivery_changes_nothing() {
        let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
        latest.apply_command(CommandOutcome::Applied {
            expected_snapshot: CountedSnapshot(5),
        });
        assert!(matches!(latest, SubscribedSnapshot::AwaitingFirstSnapshot));
    }

    #[test]
    fn a_refused_command_shows_its_failure_with_the_held_timing() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), timeline.at(10));
        latest.apply_command(CommandOutcome::Refused {
            refusal: CountingFailure,
        });
        assert!(latest.failure().is_some());
        assert_eq!(
            latest.delivery_timing(),
            Some(DeliveryStamp {
                cadence: DeliveryCadence::PeriodicPolling(PeriodicPolling::every(
                    Duration::from_secs(3)
                )),
                delivered_at: timeline.at(10).instant,
            })
        );
    }

    #[test]
    fn going_offline_keeps_the_held_snapshot_normal_until_its_poll_period_expires() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), timeline.at(10));
        latest.receive(
            SnapshotDelivery::InternetUnreachable(InternetUnreachable::NoConnection),
            timeline.at(11),
        );
        assert_eq!(latest.get(), Some(&CountedSnapshot(1)));
        assert_eq!(
            latest.display().map(|widget| widget.state(timeline.at(12))),
            Some(ReadingDisplayState::Idle)
        );
        assert_eq!(
            latest.display().map(|widget| widget.state(timeline.at(13))),
            Some(ReadingDisplayState::Offline)
        );
        latest.receive(polled_delivery(Ok(CountedSnapshot(2))), timeline.at(16));
        assert!(matches!(latest, SubscribedSnapshot::SnapshotHeld { .. }));
    }

    #[test]
    fn going_offline_before_any_snapshot_shows_a_placeholder() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
        latest.receive(
            SnapshotDelivery::InternetUnreachable(InternetUnreachable::WirelessRadioOff),
            timeline.at(10),
        );
        assert!(latest.get().is_none());
        assert!(latest.display().is_some());
        assert!(!latest.hover_card(timeline.at(10)).is_empty());
    }
}
