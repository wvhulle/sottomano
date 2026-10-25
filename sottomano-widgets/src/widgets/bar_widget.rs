use chrono::{DateTime, Locale, Utc};
use iced::{Color, Element};
use sottomano_config::severity::Severity;
use sottomano_sources::ai_usage_spend::usage_quota::Quota;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        announced_level::AnnouncedLevel,
        phosphor::{Phosphor, Weight, view_phosphor},
        simple_icons::{SimpleIcon, view_simple_icon},
        tooltip_body::TooltipBody,
        weather_icons::WeatherIcon,
        widget_state::WidgetState,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Glyph {
    Phosphor(Phosphor),
    SimpleIcon(SimpleIcon),
    WeatherIcon(WeatherIcon),
}

impl Glyph {
    #[must_use]
    pub const fn with_weight(self, weight: Weight) -> Self {
        match self {
            Glyph::Phosphor(icon) => Glyph::Phosphor(icon.with_weight(weight)),
            Glyph::SimpleIcon(_) | Glyph::WeatherIcon(_) => self,
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(self, size: f32, color: Color) -> Element<'a, Message> {
        match self {
            Glyph::Phosphor(icon) => view_phosphor(icon, size, color),
            Glyph::SimpleIcon(icon) => view_simple_icon(icon, size, color),
            Glyph::WeatherIcon(icon) => icon.view(size, color),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OngoingToastThreshold {
    NeverNotify,
    NotifyFromSeverity {
        minimum_severity: Severity,
        summary_text: &'static str,
    },
}

pub trait BarWidget {
    fn name(&self) -> &'static str;

    fn icon(&self) -> Glyph;

    #[must_use]
    fn fallback_icon() -> Glyph
    where
        Self: Sized,
    {
        Phosphor::Question(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        None
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::Idle
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

    fn live_gauge(&self) -> Option<Ratio> {
        self.gauge()
    }

    fn announced_level(&self, _locale: Locale) -> Option<AnnouncedLevel> {
        None
    }

    fn visible_in_bar(&self) -> bool {
        true
    }

    fn quota_windows(&self, _now: DateTime<Utc>) -> Vec<Quota> {
        Vec::new()
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody;

    fn icon_at(&self, clock: Clock) -> Glyph {
        self.icon().with_weight(self.state(clock).weight())
    }
}

impl<W: BarWidget + ?Sized> BarWidget for &W {
    fn name(&self) -> &'static str {
        (**self).name()
    }

    fn icon(&self) -> Glyph {
        (**self).icon()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        (**self).label(clock, locale)
    }

    fn state(&self, clock: Clock) -> WidgetState {
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

    fn live_gauge(&self) -> Option<Ratio> {
        (**self).live_gauge()
    }

    fn announced_level(&self, locale: Locale) -> Option<AnnouncedLevel> {
        (**self).announced_level(locale)
    }

    fn visible_in_bar(&self) -> bool {
        (**self).visible_in_bar()
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        (**self).quota_windows(now)
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        (**self).tooltip(clock)
    }
}
