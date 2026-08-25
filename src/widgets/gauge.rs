use iced::{
    Border, Color, Element,
    widget::{ProgressBar, progress_bar, stack},
};

const TRACK_ALPHA: f32 = 0.27;
const HEADROOM_ALPHA: f32 = 0.5;
const GIRTH: f32 = 3.0;

pub struct Gauge {
    percent: i64,
    color: Color,
    height: f32,
    live: f32,
}

impl Gauge {
    pub fn new(percent: i64, color: Color, height: f32) -> Self {
        Self {
            percent,
            color,
            height,
            live: 1.0,
        }
    }

    #[must_use]
    pub fn live(mut self, fraction: f32) -> Self {
        self.live = fraction.clamp(0.0, 1.0);
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

impl<'a, Message: 'a> From<Gauge> for Element<'a, Message> {
    fn from(gauge: Gauge) -> Self {
        let track = gauge.color.scale_alpha(TRACK_ALPHA);
        let reach = gauge.percent as f32 / 100.0;
        if gauge.live >= 1.0 {
            return gauge.bar(reach, track, gauge.color).into();
        }
        stack![
            gauge.bar(reach, track, gauge.color.scale_alpha(HEADROOM_ALPHA)),
            gauge.bar(reach * gauge.live, Color::TRANSPARENT, gauge.color),
        ]
        .into()
    }
}
