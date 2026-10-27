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

const TRACK_ALPHA: f32 = 0.27;
const HEADROOM_ALPHA: f32 = 0.5;
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
    color: Color,
    height: f32,
    live: Ratio,
}

impl Gauge {
    #[must_use]
    pub fn new(fill: Ratio, color: Color, height: f32) -> Self {
        Self {
            fill,
            color,
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
        track_color: Color,
        bar_color: Color,
    ) -> Element<'a, Message> {
        progress_bar(0.0..=1.0, reach)
            .vertical()
            .length(self.height)
            .girth(GIRTH)
            .style(move |_: &Theme| progress_bar::Style {
                background: track_color.into(),
                bar: bar_color.into(),
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
        stack![
            gauge.vertical_bar(
                reach,
                gauge.color.scale_alpha(TRACK_ALPHA),
                gauge.color.scale_alpha(HEADROOM_ALPHA),
            ),
            gauge.vertical_bar(live_reach, Color::TRANSPARENT, gauge.color),
        ]
        .into()
    }
}
