use std::marker::PhantomData;

use chrono::{DateTime, Locale, Utc};
use sottomano_config::severity::Severity;
use sottomano_sources::{
    SnapshotSource, ai_usage_spend::usage_quota::Quota,
    network_connectivity::internet_connectivity_gate::InternetUnreachable,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        reading_display_state::ReadingDisplayState,
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
    },
};

const AWAITING_SNAPSHOT_LABEL: &str = "Loading…";

impl From<InternetUnreachable> for Section {
    fn from(offline_reason: InternetUnreachable) -> Self {
        Section::dictionary([
            LabeledValue::new("Offline", offline_reason.to_string()).level(Severity::Warning)
        ])
    }
}

pub struct OfflineSnapshot<S> {
    pub held_snapshot: S,
    pub offline_reason: Box<InternetUnreachable>,
}

impl<S: GroupItemReading> GroupItemReading for OfflineSnapshot<S> {
    fn name(&self) -> &'static str {
        self.held_snapshot.name()
    }

    fn icon(&self) -> IconGlyph {
        self.held_snapshot.icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.held_snapshot.label(clock, locale)
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Off
    }

    fn gauge(&self) -> Option<Ratio> {
        self.held_snapshot.gauge()
    }

    fn badge(&self) -> Option<u32> {
        self.held_snapshot.badge()
    }

    fn identity_ordinal(&self) -> Option<u8> {
        self.held_snapshot.identity_ordinal()
    }

    fn visible_on_panel(&self) -> bool {
        self.held_snapshot.visible_on_panel()
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.held_snapshot.quota_windows(now)
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        self.held_snapshot
            .tooltip(clock)
            .with_sections([(*self.offline_reason).clone().into()])
    }
}

pub enum PlaceholderCause {
    Offline(InternetUnreachable),
    AwaitingFirstSnapshot,
}

pub struct SnapshotPlaceholder<S> {
    pub placeholder_cause: PlaceholderCause,
    pub placeholder_source: PhantomData<S>,
}

impl<S: SnapshotSource + GroupItemReading> GroupItemReading for SnapshotPlaceholder<S> {
    fn name(&self) -> &'static str {
        S::NAME
    }

    fn icon(&self) -> IconGlyph {
        S::fallback_icon()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self.placeholder_cause {
            PlaceholderCause::Offline(_) => None,
            PlaceholderCause::AwaitingFirstSnapshot => Some(AWAITING_SNAPSHOT_LABEL.to_owned()),
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Off
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([match &self.placeholder_cause {
            PlaceholderCause::Offline(offline_reason) => offline_reason.clone().into(),
            PlaceholderCause::AwaitingFirstSnapshot => Section::note(AWAITING_SNAPSHOT_LABEL),
        }])
    }
}
