mod slider_face;

use std::ops::RangeInclusive;

use iced::{
    Color, Element, Size,
    mouse::ScrollDelta,
    widget::{Slider, mouse_area, slider, stack},
};

use self::slider_face::{FaceGeometry, SliderFace};
use crate::{
    theming::{ThemedColor, primary_color, style_metrics::ACCENT_STROKE_PX, text_color},
    value_controls::SourceLag,
};

const SLIDER_WIDTH_PX: f32 = 60.0;

const DEFAULT_ICON_SIZE_PX: f32 = 14.0;
const DEFAULT_STEP: f32 = 1.0;

pub struct InlineSlider<'view, Message> {
    range: RangeInclusive<f32>,
    value: f32,
    on_change: Box<dyn Fn(f32) -> Message + 'view>,
    icon_size_px: f32,
    stroke_width_px: f32,
    source_lag: SourceLag,
    step: f32,
    accents: SliderAccents<'view>,
    release_report: ReleaseReport<Message>,
    scroll_report: ScrollReport<'view, Message>,
}

enum ReleaseReport<Message> {
    Silent,
    Reported(Message),
}

enum ScrollReport<'view, Message> {
    Ignored,
    Reported(Box<dyn Fn(ScrollDelta) -> Message + 'view>),
}

pub fn inline_slider<'view, Message: Clone + 'view>(
    range: RangeInclusive<f32>,
    value: f32,
    on_change: impl Fn(f32) -> Message + 'view,
) -> InlineSlider<'view, Message> {
    InlineSlider {
        range,
        value,
        on_change: Box::new(on_change),
        icon_size_px: DEFAULT_ICON_SIZE_PX,
        stroke_width_px: ACCENT_STROKE_PX,
        source_lag: SourceLag::CaughtUp,
        step: DEFAULT_STEP,
        accents: SliderAccents::default(),
        release_report: ReleaseReport::Silent,
        scroll_report: ScrollReport::Ignored,
    }
}

impl<'view, Message> InlineSlider<'view, Message> {
    #[must_use]
    pub fn icon_size(self, icon_size_px: f32) -> Self {
        Self {
            icon_size_px,
            ..self
        }
    }

    #[must_use]
    pub fn stroke_width(self, stroke_width_px: f32) -> Self {
        Self {
            stroke_width_px,
            ..self
        }
    }

    #[must_use]
    pub fn lagging_behind(self, source_lag: SourceLag) -> Self {
        Self { source_lag, ..self }
    }

    #[must_use]
    pub fn step(self, step: f32) -> Self {
        Self { step, ..self }
    }

    #[must_use]
    pub fn accents(self, accents: SliderAccents<'view>) -> Self {
        Self { accents, ..self }
    }

    #[must_use]
    pub fn on_release(self, on_release: Message) -> Self {
        Self {
            release_report: ReleaseReport::Reported(on_release),
            ..self
        }
    }

    #[must_use]
    pub fn on_scroll(self, on_scroll: impl Fn(ScrollDelta) -> Message + 'view) -> Self {
        Self {
            scroll_report: ScrollReport::Reported(Box::new(on_scroll)),
            ..self
        }
    }
}

impl<'view, Message: Clone + 'view> From<InlineSlider<'view, Message>> for Element<'view, Message> {
    fn from(inline: InlineSlider<'view, Message>) -> Self {
        let geometry = FaceGeometry::scaled_to_icon(inline.icon_size_px, inline.stroke_width_px);
        let face = SliderFace {
            range: inline.range.clone(),
            value: inline.value,
            source_lag: inline.source_lag,
            accents: inline.accents,
            geometry,
            face_size: Size::new(SLIDER_WIDTH_PX, Slider::<f32, Message>::DEFAULT_HEIGHT),
        };
        let interaction = interaction_slider(
            geometry,
            inline.range,
            inline.value,
            inline.step,
            inline.on_change,
            inline.release_report,
        );
        let stacked = mouse_area(stack![interaction, Element::new(face)]);
        match inline.scroll_report {
            ScrollReport::Reported(on_scroll) => stacked.on_scroll(on_scroll).into(),
            ScrollReport::Ignored => stacked.into(),
        }
    }
}

fn interaction_slider<'view, Message: Clone + 'view>(
    geometry: FaceGeometry,
    range: RangeInclusive<f32>,
    value: f32,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'view,
    release_report: ReleaseReport<Message>,
) -> Slider<'view, f32, Message> {
    let styled = slider(range, value, on_change)
        .step(step)
        .width(SLIDER_WIDTH_PX)
        .style(move |_theme, _status| slider::Style {
            rail: slider::Rail {
                backgrounds: (Color::TRANSPARENT.into(), Color::TRANSPARENT.into()),
                width: geometry.stroke_width,
                border: iced::Border::default(),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle {
                    radius: geometry.edge_inset(),
                },
                background: Color::TRANSPARENT.into(),
                border_color: Color::TRANSPARENT,
                border_width: 0.0,
            },
        });
    match release_report {
        ReleaseReport::Reported(on_release) => styled.on_release(on_release),
        ReleaseReport::Silent => styled,
    }
}

pub struct SliderAccents<'view> {
    pub resting: ThemedColor<'view>,
    pub engaged: ThemedColor<'view>,
}

impl Default for SliderAccents<'_> {
    fn default() -> Self {
        Self {
            resting: Box::new(text_color),
            engaged: Box::new(primary_color),
        }
    }
}
