use iced::{
    Color, Element, Theme, border,
    widget::{progress_bar, stack},
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::Ratio,
        ratio::{percent, ratio},
    },
};

use crate::{
    group_items::reading_display_state::ReadingStateTint,
    widgets::configured_theme::{background_strong_color, secondary_text_color},
};

const GIRTH: f32 = 3.0;

#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn percent_fill(whole_percent: i64) -> Ratio {
    Ratio::new::<percent>(whole_percent as f64)
}

#[must_use]
pub fn percent_text(fraction: Ratio) -> String {
    format!("{:.0}", fraction.into_format_args(percent, Abbreviation))
}

pub struct Gauge {
    fill: Ratio,
    tint: ReadingStateTint,
    height: f32,
    live: Ratio,
}

impl Gauge {
    #[must_use]
    pub fn new(fill: Ratio, tint: ReadingStateTint, height: f32) -> Self {
        Self {
            fill,
            tint,
            height,
            live: fill,
        }
    }

    #[must_use]
    pub fn live(mut self, live: Ratio) -> Self {
        self.live = live;
        self
    }

    fn vertical_bar<'a, Message: 'a>(
        &self,
        reach: f32,
        track_color: impl Fn(&Theme) -> Color + 'a,
        bar_color: impl Fn(&Theme) -> Color + 'a,
    ) -> Element<'a, Message> {
        progress_bar(0.0..=1.0, reach)
            .vertical()
            .length(self.height)
            .girth(GIRTH)
            .style(move |theme: &Theme| progress_bar::Style {
                background: track_color(theme).into(),
                bar: bar_color(theme).into(),
                border: border::rounded(1),
            })
            .into()
    }
}

#[allow(clippy::cast_possible_truncation)]
fn bar_reach(fill: Ratio) -> f32 {
    fill.get::<ratio>() as f32
}

impl<'a, Message: 'a> From<Gauge> for Element<'a, Message> {
    fn from(gauge: Gauge) -> Self {
        let reach = bar_reach(gauge.fill);
        let live_reach = bar_reach(gauge.live).min(reach);
        let tint = gauge.tint;
        stack![
            gauge.vertical_bar(reach, background_strong_color, secondary_text_color),
            gauge.vertical_bar(
                live_reach,
                |_: &Theme| Color::TRANSPARENT,
                move |theme: &Theme| tint.color(theme),
            ),
        ]
        .into()
    }
}
