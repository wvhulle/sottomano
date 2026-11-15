use std::{borrow::Cow, marker::PhantomData};

use chrono::{DateTime, Locale, Utc};
use sottomano_desktop_sources::{
    SnapshotSource, ai_usage_spend::usage_quota::Quota,
    network_connectivity::internet_connectivity_gate::InternetUnreachable,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{
        Section,
        hover_card_source_delivery::{DeliveryStamp, SnapshotFreshness, SourceDelivery},
    },
    widgets::icon_glyph::IconGlyph,
};

pub enum SnapshotTrouble {
    FetchFailed(String),
    Offline(InternetUnreachable),
}

impl SnapshotTrouble {
    fn source_delivery(&self) -> SourceDelivery {
        match self {
            SnapshotTrouble::FetchFailed(_) => SourceDelivery::Failed,
            SnapshotTrouble::Offline(_) => SourceDelivery::Offline,
        }
    }

    fn detail(&self) -> String {
        match self {
            SnapshotTrouble::FetchFailed(message) => message.clone(),
            SnapshotTrouble::Offline(offline_reason) => offline_reason.to_string(),
        }
    }
}

pub struct TroubledSnapshot<S> {
    pub held_snapshot: S,
    pub trouble: SnapshotTrouble,
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
}

impl<S: GroupItemReading> GroupItemReading for TroubledSnapshot<S> {
    fn name(&self) -> Cow<'static, str> {
        self.held_snapshot.name()
    }

    fn icon(&self) -> IconGlyph {
        self.held_snapshot.icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.held_snapshot.label(clock, locale)
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        match (&self.trouble, self.delivery_timing.freshness_at(clock)) {
            (SnapshotTrouble::FetchFailed(_), _) => ReadingDisplayState::Failed,
            (SnapshotTrouble::Offline(_), SnapshotFreshness::Fresh) => {
                self.held_snapshot.state(clock)
            }
            (SnapshotTrouble::Offline(_), SnapshotFreshness::PastPollPeriod) => {
                ReadingDisplayState::Offline
            }
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        self.held_snapshot.gauge()
    }

    fn badge(&self) -> Option<usize> {
        self.held_snapshot.badge()
    }

    fn identity_ordinal(&self) -> Option<u8> {
        self.held_snapshot.identity_ordinal()
    }

    fn panel_visibility(&self) -> PanelVisibility {
        self.held_snapshot.panel_visibility()
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.held_snapshot.quota_windows(now)
    }

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.held_snapshot.hover_card(clock)
    }
}

pub enum PlaceholderCause {
    Offline(InternetUnreachable),
    FetchFailed(String),
    AwaitingFirstSnapshot,
}

pub struct SnapshotPlaceholder<S> {
    pub placeholder_cause: PlaceholderCause,
    pub placeholder_source: PhantomData<S>,
}

impl<S: SnapshotSource> SnapshotPlaceholder<S> {
    #[must_use]
    pub fn caused_by(placeholder_cause: PlaceholderCause) -> Self {
        Self {
            placeholder_cause,
            placeholder_source: PhantomData,
        }
    }

    #[must_use]
    pub fn source_state(&self) -> Section {
        match &self.placeholder_cause {
            PlaceholderCause::Offline(offline_reason) => {
                Section::source_state::<S>(SourceDelivery::Offline, offline_reason.to_string())
            }
            PlaceholderCause::FetchFailed(message) => {
                Section::source_state::<S>(SourceDelivery::Failed, message.clone())
            }
            PlaceholderCause::AwaitingFirstSnapshot => {
                Section::source_state::<S>(SourceDelivery::Loading, "")
            }
        }
    }
}

impl<S: SnapshotSource + GroupItemReading> GroupItemReading for SnapshotPlaceholder<S> {
    fn name(&self) -> Cow<'static, str> {
        S::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        S::fallback_icon()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.placeholder_cause {
            PlaceholderCause::Offline(_) => ReadingDisplayState::Offline,
            PlaceholderCause::FetchFailed(_) => ReadingDisplayState::Failed,
            PlaceholderCause::AwaitingFirstSnapshot => ReadingDisplayState::Pending,
        }
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        Vec::new()
    }
}
