use chrono::{DateTime, Locale, Utc};
use iced::{Color, Element};
use sottomano_config::severity::Severity;
use sottomano_sources::ai::quota::Quota;
use uom::si::f64::Ratio;

use crate::widgets::{
    announced_level::AnnouncedLevel,
    ongoing_toast::OngoingBody,
    phosphor::{Phosphor, Weight},
    simple_icons::SimpleIcon,
    tooltip_body::TooltipBody,
    weather_icons::WeatherIcon,
    widget_state::WidgetState,
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
            Glyph::Phosphor(icon) => icon.view(size, color),
            Glyph::SimpleIcon(icon) => icon.view(size, color),
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

    fn label(&self) -> Option<String> {
        None
    }

    fn localized_label(&self, _locale: Locale) -> Option<String> {
        self.label()
    }

    fn state(&self) -> WidgetState {
        WidgetState::Idle
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NeverNotify
    }

    fn ongoing_toast_body(&self, locale: Locale) -> OngoingBody {
        self.localized_label(locale)
            .map_or(OngoingBody::None, OngoingBody::Text)
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

    fn announced_level(&self) -> Option<AnnouncedLevel> {
        None
    }

    fn visible_in_bar(&self) -> bool {
        true
    }

    fn quota_windows(&self, _now: DateTime<Utc>) -> Vec<Quota> {
        Vec::new()
    }

    fn tooltip(&self) -> TooltipBody;
}

impl<W: BarWidget + ?Sized> BarWidget for &W {
    fn name(&self) -> &'static str {
        (**self).name()
    }

    fn icon(&self) -> Glyph {
        (**self).icon()
    }

    fn label(&self) -> Option<String> {
        (**self).label()
    }

    fn localized_label(&self, locale: Locale) -> Option<String> {
        (**self).localized_label(locale)
    }

    fn state(&self) -> WidgetState {
        (**self).state()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        (**self).ongoing_toast_threshold()
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

    fn announced_level(&self) -> Option<AnnouncedLevel> {
        (**self).announced_level()
    }

    fn visible_in_bar(&self) -> bool {
        (**self).visible_in_bar()
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        (**self).quota_windows(now)
    }

    fn tooltip(&self) -> TooltipBody {
        (**self).tooltip()
    }
}
