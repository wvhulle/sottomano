use chrono::{DateTime, Locale, Utc};
use sottomano_config::severity::Severity;
use sottomano_sources::ai_usage_spend::usage_quota::Quota;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        announced_level::AnnouncedLevel,
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        tooltip_body::TooltipBody,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OngoingToastThreshold {
    NeverNotify,
    NotifyFromSeverity {
        minimum_severity: Severity,
        summary_text: &'static str,
    },
}

pub trait GroupItemReading {
    fn name(&self) -> &'static str;

    fn icon(&self) -> IconGlyph;

    #[must_use]
    fn fallback_icon() -> IconGlyph
    where
        Self: Sized,
    {
        Phosphor::Question(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        None
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Idle
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NeverNotify
    }

    fn ongoing_toast_body(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.label(clock, locale)
    }

    fn gauge(&self) -> Option<Ratio> {
        None
    }

    fn badge(&self) -> Option<u32> {
        None
    }

    fn identity_ordinal(&self) -> Option<u8> {
        None
    }

    fn live_gauge(&self) -> Option<Ratio> {
        self.gauge()
    }

    fn announced_level(&self, _locale: Locale) -> Option<AnnouncedLevel> {
        None
    }

    fn visible_on_panel(&self) -> bool {
        true
    }

    fn quota_windows(&self, _now: DateTime<Utc>) -> Vec<Quota> {
        Vec::new()
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody;

    fn icon_at(&self, clock: Clock) -> IconGlyph {
        self.icon().with_weight(self.state(clock).weight())
    }
}

impl<ForwardedReading: GroupItemReading + ?Sized> GroupItemReading for &ForwardedReading {
    fn name(&self) -> &'static str {
        (**self).name()
    }

    fn icon(&self) -> IconGlyph {
        (**self).icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        (**self).label(clock, locale)
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
        (**self).state(clock)
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        (**self).ongoing_toast_threshold()
    }

    fn ongoing_toast_body(&self, clock: Clock, locale: Locale) -> Option<String> {
        (**self).ongoing_toast_body(clock, locale)
    }

    fn gauge(&self) -> Option<Ratio> {
        (**self).gauge()
    }

    fn badge(&self) -> Option<u32> {
        (**self).badge()
    }

    fn identity_ordinal(&self) -> Option<u8> {
        (**self).identity_ordinal()
    }

    fn live_gauge(&self) -> Option<Ratio> {
        (**self).live_gauge()
    }

    fn announced_level(&self, locale: Locale) -> Option<AnnouncedLevel> {
        (**self).announced_level(locale)
    }

    fn visible_on_panel(&self) -> bool {
        (**self).visible_on_panel()
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        (**self).quota_windows(now)
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        (**self).tooltip(clock)
    }
}
