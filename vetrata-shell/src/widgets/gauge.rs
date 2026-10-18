use iced::{
    Border, Color, Element, Theme,
    widget::{Space, column, container},
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

    fn segment<'a, Message: 'a>(&self, fraction: f32, color: Color) -> Element<'a, Message> {
        container(Space::new().width(GIRTH).height(self.height * fraction))
            .style(rounded_fill(color))
            .into()
    }
}

fn rounded_fill(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(color.into()),
        border: Border::default().rounded(1.0),
        ..container::Style::default()
    }
}

#[allow(clippy::cast_possible_truncation)]
fn bar_reach(fill: Ratio) -> f32 {
    fill.get::<ratio>().clamp(0.0, 1.0) as f32
}

impl<'a, Message: 'a> From<Gauge> for Element<'a, Message> {
    fn from(gauge: Gauge) -> Self {
        let reach = bar_reach(gauge.fill);
        let live_reach = bar_reach(gauge.live).min(reach);
        container(column![
            gauge.segment(reach - live_reach, gauge.color.scale_alpha(HEADROOM_ALPHA)),
            gauge.segment(live_reach, gauge.color),
        ])
        .align_bottom(gauge.height)
        .style(rounded_fill(gauge.color.scale_alpha(TRACK_ALPHA)))
        .into()
    }
}
