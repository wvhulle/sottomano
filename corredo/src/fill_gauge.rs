use iced::{
    Color, Element, Theme,
    widget::{progress_bar, stack},
};

use crate::{
    shared_metrics::{ACCENT_STROKE_PX, pill_corners},
    theme_colors::{ThemedColor, background_strong_color, secondary_text_color},
};

pub struct FillGauge<'a> {
    fill_fraction: f32,
    live_fraction: f32,
    height: f32,
    thickness_px: f32,
    live_color: ThemedColor<'a>,
}

impl<'a> FillGauge<'a> {
    #[must_use]
    pub fn new(fill_fraction: f32, live_color: impl Fn(&Theme) -> Color + 'a, height: f32) -> Self {
        Self {
            fill_fraction,
            live_fraction: fill_fraction,
            height,
            thickness_px: ACCENT_STROKE_PX,
            live_color: Box::new(live_color),
        }
    }

    #[must_use]
    pub fn thickness(self, thickness_px: f32) -> Self {
        Self {
            thickness_px,
            ..self
        }
    }

    #[must_use]
    pub fn live(mut self, live_fraction: f32) -> Self {
        self.live_fraction = live_fraction;
        self
    }

    fn vertical_bar<Message: 'a>(
        height: f32,
        thickness_px: f32,
        reach: f32,
        track_color: impl Fn(&Theme) -> Color + 'a,
        bar_color: impl Fn(&Theme) -> Color + 'a,
    ) -> Element<'a, Message> {
        progress_bar(0.0..=1.0, reach)
            .vertical()
            .length(height)
            .girth(thickness_px)
            .style(move |theme: &Theme| progress_bar::Style {
                background: track_color(theme).into(),
                bar: bar_color(theme).into(),
                border: pill_corners(thickness_px),
            })
            .into()
    }
}

impl<'a, Message: 'a> From<FillGauge<'a>> for Element<'a, Message> {
    fn from(gauge: FillGauge<'a>) -> Self {
        let live_reach = gauge.live_fraction.min(gauge.fill_fraction);
        let live_color = gauge.live_color;
        stack![
            FillGauge::vertical_bar(
                gauge.height,
                gauge.thickness_px,
                gauge.fill_fraction,
                background_strong_color,
                secondary_text_color,
            ),
            FillGauge::vertical_bar(
                gauge.height,
                gauge.thickness_px,
                live_reach,
                |_: &Theme| Color::TRANSPARENT,
                move |theme: &Theme| live_color(theme),
            ),
        ]
        .into()
    }
}
