use iced::{
    Border, Color, Element,
    widget::{ProgressBar, progress_bar, stack},
};
use uom::si::{
    f64::Ratio,
    ratio::{percent, ratio},
};

const TRACK_ALPHA: f32 = 0.27;
const HEADROOM_ALPHA: f32 = 0.5;
const GIRTH: f32 = 3.0;

#[allow(clippy::cast_precision_loss)]
pub fn percent_fill(whole_percent: i64) -> Ratio {
    Ratio::new::<percent>(whole_percent as f64)
}

pub struct Gauge {
    fill: Ratio,
    color: Color,
    height: f32,
    live: Ratio,
}

impl Gauge {
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

    fn bar<'a>(&self, fraction: f32, track: Color, fill: Color) -> ProgressBar<'a> {
        progress_bar(0.0..=1.0, fraction)
            .vertical()
            .girth(GIRTH)
            .length(self.height)
            .style(move |_| progress_bar::Style {
                background: track.into(),
                bar: fill.into(),
                border: Border::default().rounded(1.0),
            })
    }
}

#[allow(clippy::cast_possible_truncation)]
fn bar_reach(fill: Ratio) -> f32 {
    fill.get::<ratio>().clamp(0.0, 1.0) as f32
}

impl<'a, Message: 'a> From<Gauge> for Element<'a, Message> {
    fn from(gauge: Gauge) -> Self {
        let track = gauge.color.scale_alpha(TRACK_ALPHA);
        let reach = bar_reach(gauge.fill);
        let live_reach = bar_reach(gauge.live);
        if live_reach >= reach {
            return gauge.bar(reach, track, gauge.color).into();
        }
        stack![
            gauge.bar(reach, track, gauge.color.scale_alpha(HEADROOM_ALPHA)),
            gauge.bar(live_reach, Color::TRANSPARENT, gauge.color),
        ]
        .into()
    }
}
