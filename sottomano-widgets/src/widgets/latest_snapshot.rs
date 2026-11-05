use std::{convert::Infallible, marker::PhantomData, mem::take};

use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::internet_connectivity_gate::InternetUnreachable,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        offline_internet_reading::{OfflineSnapshot, PlaceholderCause, SnapshotPlaceholder},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody, ReadingSections, Section,
            hover_card_source_delivery::{DeliveryTiming, HoverCard, SourcedSections},
        },
    },
    widgets::icon_glyph::IconGlyph,
};

pub struct FailedSnapshotFetch<S: SnapshotSource> {
    icon: IconGlyph,
    error: S::Error,
}

impl<S: SnapshotSource + GroupItemReading> FailedSnapshotFetch<S> {
    pub fn from_fetch_error(error: S::Error) -> Self {
        Self {
            icon: S::fallback_icon(),
            error,
        }
    }
}

impl<S: SnapshotSource<Error: Into<Section>>> GroupItemReading for FailedSnapshotFetch<S> {
    fn name(&self) -> &'static str {
        "Failed reading"
    }

    fn icon(&self) -> IconGlyph {
        self.icon
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([self.error.clone().into()])
    }
}

impl GroupItemReading for Infallible {
    fn name(&self) -> &'static str {
        match *self {}
    }

    fn icon(&self) -> IconGlyph {
        match *self {}
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
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
    type ShownFailure: GroupItemReading;

    fn after_failed_fetch(
        latest: LatestSnapshot<S, Self>,
        error: S::Error,
        delivery_timing: DeliveryTiming,
    ) -> LatestSnapshot<S, Self>;
}

#[derive(Default)]
pub enum LatestSnapshot<S, Policy = ShowsFetchFailures>
where
    S: SnapshotSource,
    Policy: FetchFailurePolicy<S>,
{
    #[default]
    AwaitingFirstSnapshot,
    SourceUnavailable,
    SnapshotHeld {
        held_snapshot: S,
        delivery_timing: DeliveryTiming,
    },
    SnapshotFetchFailed {
        fetch_failure: Policy::ShownFailure,
        delivery_timing: DeliveryTiming,
    },
    SnapshotHeldOffline {
        offline_snapshot: OfflineSnapshot<S>,
        delivery_timing: DeliveryTiming,
    },
    OfflineBeforeSnapshot {
        offline_placeholder: SnapshotPlaceholder<S>,
    },
}

impl<S: SnapshotSource + GroupItemReading> FetchFailurePolicy<S> for ShowsFetchFailures
where
    S::Error: Into<Section>,
{
    type ShownFailure = FailedSnapshotFetch<S>;

    fn after_failed_fetch(
        latest: LatestSnapshot<S, Self>,
        error: S::Error,
        delivery_timing: DeliveryTiming,
    ) -> LatestSnapshot<S, Self> {
        let icon = match &latest {
            LatestSnapshot::SnapshotHeld { held_snapshot, .. }
            | LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                ..
            } => held_snapshot.icon(),
            LatestSnapshot::SnapshotFetchFailed { fetch_failure, .. } => fetch_failure.icon,
            LatestSnapshot::AwaitingFirstSnapshot
            | LatestSnapshot::SourceUnavailable
            | LatestSnapshot::OfflineBeforeSnapshot { .. } => S::fallback_icon(),
        };
        LatestSnapshot::SnapshotFetchFailed {
            fetch_failure: FailedSnapshotFetch { icon, error },
            delivery_timing,
        }
    }
}

impl<S: SnapshotSource + GroupItemReading> FetchFailurePolicy<S> for HidesFailuresAfterSuccess
where
    S::Error: Into<Section>,
{
    type ShownFailure = FailedSnapshotFetch<S>;

    fn after_failed_fetch(
        latest: LatestSnapshot<S, Self>,
        error: S::Error,
        delivery_timing: DeliveryTiming,
    ) -> LatestSnapshot<S, Self> {
        match latest {
            LatestSnapshot::SnapshotHeld { held_snapshot, .. }
            | LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                ..
            } => {
                tracing::warn!("{}: {error}", S::NAME);
                LatestSnapshot::SnapshotHeld {
                    held_snapshot,
                    delivery_timing,
                }
            }
            LatestSnapshot::SnapshotFetchFailed { fetch_failure, .. } => {
                LatestSnapshot::SnapshotFetchFailed {
                    fetch_failure: FailedSnapshotFetch {
                        icon: fetch_failure.icon,
                        error,
                    },
                    delivery_timing,
                }
            }
            LatestSnapshot::AwaitingFirstSnapshot
            | LatestSnapshot::SourceUnavailable
            | LatestSnapshot::OfflineBeforeSnapshot { .. } => LatestSnapshot::SnapshotFetchFailed {
                fetch_failure: FailedSnapshotFetch {
                    icon: S::fallback_icon(),
                    error,
                },
                delivery_timing,
            },
        }
    }
}

impl<S: SnapshotSource> FetchFailurePolicy<S> for HidesFetchFailures {
    type ShownFailure = Infallible;

    fn after_failed_fetch(
        latest: LatestSnapshot<S, Self>,
        error: S::Error,
        delivery_timing: DeliveryTiming,
    ) -> LatestSnapshot<S, Self> {
        tracing::warn!("{}: {error}", S::NAME);
        match latest {
            LatestSnapshot::SnapshotHeld { held_snapshot, .. }
            | LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                ..
            } => LatestSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            },
            LatestSnapshot::SnapshotFetchFailed { fetch_failure, .. } => match fetch_failure {},
            awaiting_or_unavailable => awaiting_or_unavailable,
        }
    }
}

impl<S: SnapshotSource, Policy: FetchFailurePolicy<S>> LatestSnapshot<S, Policy> {
    pub fn get(&self) -> Option<&S> {
        match self {
            LatestSnapshot::SnapshotHeld { held_snapshot, .. }
            | LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                ..
            } => Some(held_snapshot),
            LatestSnapshot::AwaitingFirstSnapshot
            | LatestSnapshot::SourceUnavailable
            | LatestSnapshot::SnapshotFetchFailed { .. }
            | LatestSnapshot::OfflineBeforeSnapshot { .. } => None,
        }
    }

    fn delivery_timing(&self) -> Option<DeliveryTiming> {
        match self {
            LatestSnapshot::AwaitingFirstSnapshot
            | LatestSnapshot::SourceUnavailable
            | LatestSnapshot::OfflineBeforeSnapshot { .. } => None,
            LatestSnapshot::SnapshotHeld {
                delivery_timing, ..
            }
            | LatestSnapshot::SnapshotFetchFailed {
                delivery_timing, ..
            }
            | LatestSnapshot::SnapshotHeldOffline {
                delivery_timing, ..
            } => Some(*delivery_timing),
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
            SnapshotDelivery::SourceUnavailable => LatestSnapshot::SourceUnavailable,
            SnapshotDelivery::InternetUnreachable(offline_reason) => self.offline(offline_reason),
            SnapshotDelivery::SnapshotUnchanged => self.redelivered(clock),
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch,
                stamped_cadence,
            } => {
                let delivery_timing = DeliveryTiming::from_delivery_cadence(
                    stamped_cadence.delivery_cadence(),
                    clock,
                );
                match snapshot_fetch {
                    Ok(held_snapshot) => LatestSnapshot::SnapshotHeld {
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
            LatestSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            }
            | LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot { held_snapshot, .. },
                delivery_timing,
            } => LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot: OfflineSnapshot {
                    held_snapshot,
                    offline_reason: Box::new(offline_reason),
                },
                delivery_timing,
            },
            LatestSnapshot::AwaitingFirstSnapshot
            | LatestSnapshot::SourceUnavailable
            | LatestSnapshot::SnapshotFetchFailed { .. }
            | LatestSnapshot::OfflineBeforeSnapshot { .. } => {
                LatestSnapshot::OfflineBeforeSnapshot {
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
            Ok(held_snapshot) => LatestSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            },
            Err(error) => Policy::after_failed_fetch(self, error, delivery_timing),
        }
    }

    fn redelivered(self, clock: Clock) -> Self {
        match self {
            LatestSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            } => LatestSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing: delivery_timing.redelivered(clock),
            },
            LatestSnapshot::SnapshotFetchFailed {
                fetch_failure,
                delivery_timing,
            } => LatestSnapshot::SnapshotFetchFailed {
                fetch_failure,
                delivery_timing: delivery_timing.redelivered(clock),
            },
            LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot,
                delivery_timing,
            } => LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot,
                delivery_timing: delivery_timing.redelivered(clock),
            },
            LatestSnapshot::AwaitingFirstSnapshot
            | LatestSnapshot::SourceUnavailable
            | LatestSnapshot::OfflineBeforeSnapshot { .. } => self,
        }
    }

    pub fn hover_card_with(&self, body: HoverCardBody) -> Option<HoverCard> {
        match self {
            LatestSnapshot::OfflineBeforeSnapshot { .. } => Some(HoverCard::grouped([
                SourcedSections::source_awaiting_delivery::<S>(body),
            ])),
            _ => Some(HoverCard::sourced(body, self.delivery_timing()?)),
        }
    }

    fn offline_sections(&self) -> Vec<Section> {
        match self {
            LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot, ..
            } => vec![(*offline_snapshot.offline_reason).clone().into()],
            _ => Vec::new(),
        }
    }

    pub fn sections_described_by(
        &self,
        describe: impl FnOnce(&S) -> Vec<Section>,
    ) -> Option<SourcedSections> {
        let held_snapshot = self.get()?;
        Some(SourcedSections::delivered(
            HoverCardBody::sections(describe(held_snapshot)).with_sections(self.offline_sections()),
            self.delivery_timing()?,
        ))
    }

    pub fn failure(&self) -> Option<&dyn GroupItemReading> {
        match self {
            LatestSnapshot::SnapshotFetchFailed { fetch_failure, .. } => Some(fetch_failure),
            LatestSnapshot::AwaitingFirstSnapshot
            | LatestSnapshot::SourceUnavailable
            | LatestSnapshot::SnapshotHeld { .. }
            | LatestSnapshot::SnapshotHeldOffline { .. }
            | LatestSnapshot::OfflineBeforeSnapshot { .. } => None,
        }
    }
}

impl<S: SnapshotSource + ReadingSections, Policy: FetchFailurePolicy<S>> LatestSnapshot<S, Policy> {
    pub fn sections(&self, clock: Clock) -> Option<SourcedSections> {
        let held_snapshot = self.get()?;
        Some(SourcedSections::source_delivered::<S>(
            HoverCardBody::sections(held_snapshot.sections(clock))
                .with_sections(self.offline_sections()),
            self.delivery_timing()?,
        ))
    }
}

fn awaiting_snapshot_placeholder<S>() -> SnapshotPlaceholder<S> {
    SnapshotPlaceholder {
        placeholder_cause: PlaceholderCause::AwaitingFirstSnapshot,
        placeholder_source: PhantomData,
    }
}

impl<S: SnapshotSource + GroupItemReading, Policy: FetchFailurePolicy<S>>
    LatestSnapshot<S, Policy>
{
    pub fn visible_reading(&self) -> Option<&S> {
        self.get()
            .filter(|held_snapshot| S::visible_on_panel(held_snapshot))
    }

    pub fn display(&self) -> Option<&dyn GroupItemReading> {
        match self {
            LatestSnapshot::SnapshotHeld { held_snapshot, .. } => held_snapshot
                .visible_on_panel()
                .then_some(held_snapshot as &dyn GroupItemReading),
            LatestSnapshot::SnapshotFetchFailed { fetch_failure, .. } => Some(fetch_failure),
            LatestSnapshot::SnapshotHeldOffline {
                offline_snapshot, ..
            } => offline_snapshot
                .visible_on_panel()
                .then_some(offline_snapshot as &dyn GroupItemReading),
            LatestSnapshot::OfflineBeforeSnapshot {
                offline_placeholder,
            } => Some(offline_placeholder),
            LatestSnapshot::AwaitingFirstSnapshot | LatestSnapshot::SourceUnavailable => None,
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
        if matches!(self, LatestSnapshot::AwaitingFirstSnapshot) {
            return Some(GroupItem::reading(
                awaiting_snapshot_placeholder::<S>(),
                target,
            ));
        }
        self.group_item(target)
    }

    pub fn hover_card(&self, clock: Clock) -> Option<HoverCard> {
        self.hover_card_with(self.display()?.hover_card(clock))
    }

    pub fn hover_card_while_pending(&self, clock: Clock) -> Option<HoverCard> {
        if matches!(self, LatestSnapshot::AwaitingFirstSnapshot) {
            return Some(HoverCard::grouped([
                SourcedSections::source_awaiting_delivery::<S>(
                    awaiting_snapshot_placeholder::<S>().hover_card(clock),
                ),
            ]));
        }
        self.hover_card(clock)
    }

    pub fn hover_card_with_groups(
        &self,
        extra: impl IntoIterator<Item = SourcedSections>,
        clock: Clock,
    ) -> Option<HoverCard> {
        let own = self.hover_card_with(self.display()?.hover_card(clock))?;
        Some(own.with_groups(extra))
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

    impl From<CountingFailure> for Section {
        fn from(failure: CountingFailure) -> Self {
            Section::error(failure)
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
        fn name(&self) -> &'static str {
            "Counted"
        }

        fn icon(&self) -> IconGlyph {
            Self::fallback_icon()
        }

        fn hover_card(&self, _clock: Clock) -> HoverCardBody {
            HoverCardBody::sections([])
        }
    }

    fn clock_at(seconds_since_epoch: u64) -> Clock {
        Clock {
            wall: SystemTime::UNIX_EPOCH + Duration::from_secs(seconds_since_epoch),
            ..Clock::real()
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
        let latest: LatestSnapshot<CountedSnapshot> = LatestSnapshot::default();
        assert!(matches!(latest, LatestSnapshot::AwaitingFirstSnapshot));
        assert!(latest.delivery_timing().is_none());
    }

    #[test]
    fn an_unchanged_snapshot_keeps_its_poll_period_and_moves_its_delivery_time() {
        let mut latest: LatestSnapshot<CountedSnapshot> = LatestSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), clock_at(10));
        latest.receive(SnapshotDelivery::SnapshotUnchanged, clock_at(13));
        assert_eq!(
            latest.delivery_timing(),
            Some(DeliveryTiming::PeriodicPolling {
                poll_period: Duration::from_secs(3),
                delivered_at: SystemTime::UNIX_EPOCH + Duration::from_secs(13),
            })
        );
    }

    #[test]
    fn a_holder_that_hides_failures_keeps_its_snapshot() {
        let mut latest: LatestSnapshot<CountedSnapshot, HidesFetchFailures> =
            LatestSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), clock_at(10));
        latest.receive(polled_delivery(Err(CountingFailure)), clock_at(13));
        assert_eq!(latest.get(), Some(&CountedSnapshot(1)));
    }

    #[test]
    fn failures_after_a_success_are_hidden_but_failures_before_it_are_shown() {
        let mut latest: LatestSnapshot<CountedSnapshot, HidesFailuresAfterSuccess> =
            LatestSnapshot::default();
        latest.receive(polled_delivery(Err(CountingFailure)), clock_at(10));
        assert!(latest.failure().is_some());
        latest.receive(polled_delivery(Ok(CountedSnapshot(2))), clock_at(13));
        latest.receive(polled_delivery(Err(CountingFailure)), clock_at(16));
        assert_eq!(latest.get(), Some(&CountedSnapshot(2)));
    }

    #[test]
    fn a_command_before_any_delivery_changes_nothing() {
        let mut latest: LatestSnapshot<CountedSnapshot> = LatestSnapshot::default();
        latest.apply_command(CommandOutcome::Applied {
            expected_snapshot: CountedSnapshot(5),
        });
        assert!(matches!(latest, LatestSnapshot::AwaitingFirstSnapshot));
    }

    #[test]
    fn a_refused_command_shows_its_failure_with_the_held_timing() {
        let mut latest: LatestSnapshot<CountedSnapshot> = LatestSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), clock_at(10));
        latest.apply_command(CommandOutcome::Refused {
            refusal: CountingFailure,
        });
        assert!(latest.failure().is_some());
        assert_eq!(
            latest.delivery_timing(),
            Some(DeliveryTiming::PeriodicPolling {
                poll_period: Duration::from_secs(3),
                delivered_at: SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            })
        );
    }

    #[test]
    fn going_offline_keeps_the_held_snapshot_with_its_reason() {
        let mut latest: LatestSnapshot<CountedSnapshot> = LatestSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), clock_at(10));
        latest.receive(
            SnapshotDelivery::InternetUnreachable(InternetUnreachable::NoConnection),
            clock_at(13),
        );
        assert_eq!(latest.get(), Some(&CountedSnapshot(1)));
        assert_eq!(
            latest.display().map(|widget| widget.state(clock_at(13))),
            Some(ReadingDisplayState::Off)
        );
        latest.receive(polled_delivery(Ok(CountedSnapshot(2))), clock_at(16));
        assert!(matches!(latest, LatestSnapshot::SnapshotHeld { .. }));
    }

    #[test]
    fn going_offline_before_any_snapshot_shows_a_placeholder() {
        let mut latest: LatestSnapshot<CountedSnapshot> = LatestSnapshot::default();
        latest.receive(
            SnapshotDelivery::InternetUnreachable(InternetUnreachable::WirelessRadioOff),
            clock_at(10),
        );
        assert!(latest.get().is_none());
        assert!(latest.display().is_some());
        assert!(latest.hover_card(clock_at(10)).is_some());
    }
}
