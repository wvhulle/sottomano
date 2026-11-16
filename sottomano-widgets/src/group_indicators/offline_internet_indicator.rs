use std::fmt::Display;

use sottomano_desktop_sources::{
    SnapshotSource, network_connectivity::internet_connectivity_gate::InternetUnreachable,
};

use crate::{
    clock::Clock,
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    hover_cards::hover_card_body::{
        Section,
        hover_card_source_delivery::{DeliveryStamp, SnapshotFreshness, SourceDelivery},
    },
    widgets::icon_glyph::IconGlyph,
};

pub enum SnapshotTrouble<FetchFailure> {
    FetchFailed(FetchFailure),
    Offline(InternetUnreachable),
}

impl<FetchFailure: Display> SnapshotTrouble<FetchFailure> {
    fn source_delivery(&self) -> SourceDelivery {
        match self {
            SnapshotTrouble::FetchFailed(_) => SourceDelivery::Failed,
            SnapshotTrouble::Offline(_) => SourceDelivery::Offline,
        }
    }

    fn detail(&self) -> String {
        match self {
            SnapshotTrouble::FetchFailed(fetch_failure) => fetch_failure.to_string(),
            SnapshotTrouble::Offline(offline_reason) => offline_reason.to_string(),
        }
    }
}

pub struct TroubledSnapshot<S: SnapshotSource> {
    pub held_snapshot: S,
    pub trouble: SnapshotTrouble<S::Error>,
    pub delivery_timing: DeliveryStamp,
}

impl<S: SnapshotSource> TroubledSnapshot<S> {
    pub fn trouble_section(&self) -> Section {
        Section::source_state::<S>(self.trouble.source_delivery(), self.trouble.detail())
    }

    #[must_use]
    pub fn source_delivery(&self) -> SourceDelivery {
        self.trouble.source_delivery()
    }

    #[must_use]
    pub fn shown_over<'r>(
        &self,
        described_row: GroupIndicator<'r>,
        clock: Clock,
    ) -> GroupIndicator<'r> {
        let held_state = described_row.shown_state();
        described_row.in_state(self.shown_state_over(held_state, clock))
    }

    #[must_use]
    pub fn shown_state_over(&self, held_state: IndicatorState, clock: Clock) -> IndicatorState {
        match &self.trouble {
            SnapshotTrouble::FetchFailed(_) => IndicatorState::Failed,
            SnapshotTrouble::Offline(_) => match self.delivery_timing.freshness_at(clock) {
                SnapshotFreshness::Fresh => held_state,
                SnapshotFreshness::PastPollPeriod => IndicatorState::Offline,
            },
        }
    }
}

pub enum PlaceholderCause<FetchFailure> {
    Troubled(SnapshotTrouble<FetchFailure>),
    AwaitingFirstSnapshot,
}

pub struct SnapshotPlaceholder<S: SnapshotSource> {
    pub placeholder_cause: PlaceholderCause<S::Error>,
}

impl<S: SnapshotSource> SnapshotPlaceholder<S> {
    #[must_use]
    pub fn caused_by(placeholder_cause: PlaceholderCause<S::Error>) -> Self {
        Self { placeholder_cause }
    }

    #[must_use]
    pub fn awaiting_indicator<'r>(fallback_icon: IconGlyph) -> GroupIndicator<'r>
    where
        S: 'r,
    {
        let awaiting_placeholder = Self::caused_by(PlaceholderCause::AwaitingFirstSnapshot);
        GroupIndicator::glyph_only(S::NAME, fallback_icon, IndicatorState::Pending)
            .with_hover_card(Box::new(move |_| vec![awaiting_placeholder.source_state()]))
    }

    #[must_use]
    pub fn indicator(&self, fallback_icon: IconGlyph) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(S::NAME, fallback_icon, self.shown_state())
            .with_hover_card(Box::new(move |_| vec![self.source_state()]))
    }

    #[must_use]
    pub fn shown_state(&self) -> IndicatorState {
        match self.placeholder_cause {
            PlaceholderCause::Troubled(SnapshotTrouble::Offline(_)) => IndicatorState::Offline,
            PlaceholderCause::Troubled(SnapshotTrouble::FetchFailed(_)) => IndicatorState::Failed,
            PlaceholderCause::AwaitingFirstSnapshot => IndicatorState::Pending,
        }
    }

    #[must_use]
    pub fn source_state(&self) -> Section {
        match &self.placeholder_cause {
            PlaceholderCause::Troubled(trouble) => {
                Section::source_state::<S>(trouble.source_delivery(), trouble.detail())
            }
            PlaceholderCause::AwaitingFirstSnapshot => {
                Section::source_state::<S>(SourceDelivery::Loading, "")
            }
        }
    }
}
