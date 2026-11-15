use std::{borrow::Cow, fmt::Display};

use chrono::{DateTime, Locale, Utc};
use sottomano_desktop_sources::{
    SnapshotSource, ai_usage_spend::usage_quota::Quota,
    network_connectivity::internet_connectivity_gate::InternetUnreachable,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, OngoingToastThreshold, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{
        Section,
        hover_card_source_delivery::{DeliveryStamp, SnapshotFreshness, SourceDelivery},
    },
    widgets::{announced_level::AnnouncedLevel, icon_glyph::IconGlyph},
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
}

impl<S: SnapshotSource + GroupItemReading> GroupItemReading for TroubledSnapshot<S> {
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

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        self.held_snapshot.ongoing_toast_threshold()
    }

    fn ongoing_toast_body(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.held_snapshot.ongoing_toast_body(clock, locale)
    }

    fn live_gauge(&self) -> Option<Ratio> {
        self.held_snapshot.live_gauge()
    }

    fn announced_level(&self, locale: Locale) -> Option<AnnouncedLevel> {
        self.held_snapshot.announced_level(locale)
    }

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.held_snapshot.hover_card(clock)
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

impl<S: SnapshotSource + GroupItemReading> GroupItemReading for SnapshotPlaceholder<S> {
    fn name(&self) -> Cow<'static, str> {
        S::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        S::fallback_icon()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.placeholder_cause {
            PlaceholderCause::Troubled(SnapshotTrouble::Offline(_)) => ReadingDisplayState::Offline,
            PlaceholderCause::Troubled(SnapshotTrouble::FetchFailed(_)) => {
                ReadingDisplayState::Failed
            }
            PlaceholderCause::AwaitingFirstSnapshot => ReadingDisplayState::Pending,
        }
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        Vec::new()
    }
}
