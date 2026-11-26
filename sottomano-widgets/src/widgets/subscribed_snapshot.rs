use std::mem::take;

use sottomano_desktop_sources::{SnapshotDelivery, SnapshotSource};

use crate::{
    clock::Clock,
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
        snapshot_absence::{AbsenceCause, Staleness, failed_source_indicator},
    },
    hover_cards::hover_card_body::{
        Section,
        hover_card_source_delivery::{DeliveryStamp, SourceDelivery},
    },
    indicator_groups::indicator_group_content::IndicatorListing,
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

    pub fn indicator<'view>(
        &'view self,
        describe: impl FnOnce(&'view S) -> GroupIndicator<'view>,
        absent_glyph: IconGlyph,
    ) -> GroupIndicator<'view> {
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

    pub fn indicator_listing<'view>(
        &'view self,
        describe: impl FnOnce(&'view S) -> Vec<GroupIndicator<'view>>,
        absent_glyph: IconGlyph,
    ) -> IndicatorListing<'view> {
        match self {
            SubscribedSnapshot::Available {
                snapshot,
                staleness: Staleness::Current,
                ..
            } => IndicatorListing::Listed(describe(snapshot)),
            SubscribedSnapshot::Available {
                snapshot,
                staleness: Staleness::StaleAfterFailure(fetch_failure),
                ..
            } => IndicatorListing::Incomplete {
                listed_indicators: describe(snapshot),
                unread_sources: vec![failed_source_indicator::<S>(fetch_failure, absent_glyph)],
            },
            SubscribedSnapshot::Unavailable { cause } => IndicatorListing::Incomplete {
                listed_indicators: Vec::new(),
                unread_sources: vec![cause.indicator(absent_glyph)],
            },
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
mod subscribed_snapshot_tests;
