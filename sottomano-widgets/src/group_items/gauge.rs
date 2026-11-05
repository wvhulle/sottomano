use corredo::fill_gauge::FillGauge;
use iced::{Element, Theme};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::Ratio,
        ratio::{percent, ratio},
    },
};

use crate::group_items::reading_display_state::ReadingStateTint;

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
}

#[allow(clippy::cast_possible_truncation)]
fn bar_reach(fill: Ratio) -> f32 {
    fill.get::<ratio>() as f32
}

impl<'a, Message: 'a> From<Gauge> for Element<'a, Message> {
    fn from(gauge: Gauge) -> Self {
        let tint = gauge.tint;
        FillGauge::new(
            bar_reach(gauge.fill),
            move |theme: &Theme| tint.color(theme),
            gauge.height,
        )
        .live(bar_reach(gauge.live))
        .into()
    }
}
