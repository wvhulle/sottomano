use std::mem::take;

use sottomano_desktop_sources::{SnapshotDelivery, SnapshotSource};

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::{GroupItemReading, PanelVisibility},
        offline_internet_reading::{
            PlaceholderCause, SnapshotPlaceholder, SnapshotTrouble, TroubledSnapshot,
        },
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            ReadingSections, Section,
            hover_card_source_delivery::{DeliveryStamp, SourceDelivery},
        },
    },
};

pub enum CommandOutcome<S: SnapshotSource> {
    Applied { expected_snapshot: S },
    Refused { refusal: S::Error },
    Reread { reread_fetch: Result<S, S::Error> },
}

#[derive(Default)]
pub enum SubscribedSnapshot<S: SnapshotSource> {
    #[default]
    AwaitingFirstSnapshot,
    SourceUnavailable,
    SnapshotHeld {
        held_snapshot: S,
        delivery_timing: DeliveryStamp,
    },
    SnapshotTroubled {
        troubled_snapshot: TroubledSnapshot<S>,
    },
    NoSnapshot {
        placeholder: SnapshotPlaceholder<S>,
    },
}

impl<S: SnapshotSource> SubscribedSnapshot<S> {
    pub fn get(&self) -> Option<&S> {
        match self {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. }
            | SubscribedSnapshot::SnapshotTroubled {
                troubled_snapshot: TroubledSnapshot { held_snapshot, .. },
            } => Some(held_snapshot),
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::NoSnapshot { .. } => None,
        }
    }

    fn delivery_timing(&self) -> Option<DeliveryStamp> {
        match self {
            SubscribedSnapshot::SnapshotHeld {
                delivery_timing, ..
            }
            | SubscribedSnapshot::SnapshotTroubled {
                troubled_snapshot:
                    TroubledSnapshot {
                        delivery_timing, ..
                    },
            } => Some(*delivery_timing),
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::NoSnapshot { .. } => None,
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
            SnapshotDelivery::InternetUnreachable(offline_reason) => {
                self.troubled(SnapshotTrouble::Offline(offline_reason), None)
            }
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
                    Err(fetch_error) => {
                        tracing::warn!(source = S::NAME, error = %fetch_error, "snapshot fetch failed");
                        self.troubled(
                            SnapshotTrouble::FetchFailed(fetch_error),
                            Some(delivery_timing),
                        )
                    }
                }
            }
        }
    }

    fn troubled(
        self,
        trouble: SnapshotTrouble<S::Error>,
        fresh_timing: Option<DeliveryStamp>,
    ) -> Self {
        let held = match self {
            SubscribedSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            }
            | SubscribedSnapshot::SnapshotTroubled {
                troubled_snapshot:
                    TroubledSnapshot {
                        held_snapshot,
                        delivery_timing,
                        ..
                    },
            } => Some((held_snapshot, delivery_timing)),
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::NoSnapshot { .. } => None,
        };
        let Some((held_snapshot, held_timing)) = held else {
            return SubscribedSnapshot::NoSnapshot {
                placeholder: SnapshotPlaceholder::caused_by(PlaceholderCause::Troubled(trouble)),
            };
        };
        SubscribedSnapshot::SnapshotTroubled {
            troubled_snapshot: TroubledSnapshot {
                held_snapshot,
                trouble,
                delivery_timing: fresh_timing.unwrap_or(held_timing),
            },
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
            Err(refusal) => SubscribedSnapshot::NoSnapshot {
                placeholder: SnapshotPlaceholder::caused_by(PlaceholderCause::Troubled(
                    SnapshotTrouble::FetchFailed(refusal),
                )),
            },
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
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                SubscribedSnapshot::SnapshotTroubled {
                    troubled_snapshot: TroubledSnapshot {
                        delivery_timing: troubled_snapshot.delivery_timing.redelivered(clock),
                        ..troubled_snapshot
                    },
                }
            }
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::NoSnapshot { .. } => self,
        }
    }

    pub fn sections_described_by(&self, describe: impl FnOnce(&S) -> Vec<Section>) -> Vec<Section> {
        match self {
            SubscribedSnapshot::SourceUnavailable => Vec::new(),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                vec![Section::source_state::<S>(SourceDelivery::Loading, "")]
            }
            SubscribedSnapshot::SnapshotHeld {
                held_snapshot,
                delivery_timing,
            } => describe(held_snapshot)
                .into_iter()
                .map(|section| section.stamped_by::<S>(SourceDelivery::Delivered(*delivery_timing)))
                .collect(),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let delivery = troubled_snapshot.source_delivery();
                describe(&troubled_snapshot.held_snapshot)
                    .into_iter()
                    .map(|section| section.stamped_by::<S>(delivery))
                    .chain([troubled_snapshot.trouble_section()])
                    .collect()
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => vec![placeholder.source_state()],
        }
    }

    pub fn hover_card_with(&self, sections: Vec<Section>) -> Vec<Section> {
        self.sections_described_by(|_| sections)
    }

    pub fn fetch_failure(&self) -> Option<&S::Error> {
        match self {
            SubscribedSnapshot::NoSnapshot {
                placeholder:
                    SnapshotPlaceholder {
                        placeholder_cause:
                            PlaceholderCause::Troubled(SnapshotTrouble::FetchFailed(fetch_failure)),
                    },
            } => Some(fetch_failure),
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::SnapshotHeld { .. }
            | SubscribedSnapshot::SnapshotTroubled { .. }
            | SubscribedSnapshot::NoSnapshot { .. } => None,
        }
    }
}

impl<S: SnapshotSource + GroupItemReading> SubscribedSnapshot<S> {
    pub fn failure(&self) -> Option<&dyn GroupItemReading> {
        match self {
            SubscribedSnapshot::NoSnapshot { placeholder } => Some(placeholder),
            SubscribedSnapshot::AwaitingFirstSnapshot
            | SubscribedSnapshot::SourceUnavailable
            | SubscribedSnapshot::SnapshotHeld { .. }
            | SubscribedSnapshot::SnapshotTroubled { .. } => None,
        }
    }
}

impl<S: SnapshotSource + ReadingSections> SubscribedSnapshot<S> {
    pub fn sections(&self, clock: Clock) -> Vec<Section> {
        self.sections_described_by(|held_snapshot| held_snapshot.sections(clock))
    }
}

impl<S: SnapshotSource + GroupItemReading> SubscribedSnapshot<S> {
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
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                (troubled_snapshot.panel_visibility() == PanelVisibility::Shown)
                    .then_some(troubled_snapshot as &dyn GroupItemReading)
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => Some(placeholder),
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
                SnapshotPlaceholder::<S>::caused_by(PlaceholderCause::AwaitingFirstSnapshot),
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
        borrow::Cow,
        fmt::{Display, Formatter, Result as FmtResult},
        time::{Duration, SystemTime},
    };

    use iced::futures::{Stream, stream};
    use sottomano_desktop_sources::{
        delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
        network_connectivity::internet_connectivity_gate::InternetUnreachable,
    };

    use super::*;
    use crate::{
        group_items::reading_display_state::ReadingDisplayState, widgets::icon_glyph::IconGlyph,
    };

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
    fn a_failed_fetch_keeps_the_held_snapshot_and_marks_it_failed() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), timeline.at(10));
        latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(13));
        assert_eq!(latest.get(), Some(&CountedSnapshot(1)));
        assert_eq!(
            latest.display().map(|widget| widget.state(timeline.at(13))),
            Some(ReadingDisplayState::Failed)
        );
    }

    #[test]
    fn a_failure_before_any_snapshot_shows_a_failed_placeholder() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
        latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(10));
        assert!(latest.failure().is_some());
        latest.receive(polled_delivery(Ok(CountedSnapshot(2))), timeline.at(13));
        latest.receive(polled_delivery(Err(CountingFailure)), timeline.at(16));
        assert_eq!(latest.get(), Some(&CountedSnapshot(2)));
        assert!(latest.failure().is_none());
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
    fn a_refused_command_drops_the_assumed_snapshot_and_shows_its_failure() {
        let timeline = TestTimeline::starting_now();
        let mut latest: SubscribedSnapshot<CountedSnapshot> = SubscribedSnapshot::default();
        latest.receive(polled_delivery(Ok(CountedSnapshot(1))), timeline.at(10));
        latest.apply_command(CommandOutcome::Refused {
            refusal: CountingFailure,
        });
        assert!(latest.failure().is_some());
        assert!(latest.get().is_none());
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
