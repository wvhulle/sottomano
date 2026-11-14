use std::ops::RangeInclusive;

use iced::{
    Color, Element, Event, Length, Point, Rectangle, Size, Theme,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        renderer,
        widget::{Tree, tree},
    },
    mouse::{self, Cursor, ScrollDelta},
    widget::{Slider, mouse_area, slider, stack},
};

use crate::{
    quad_shapes::{CircleQuad, HorizontalCapsuleQuad, QuadOutline},
    theming::{
        ThemedColor, background_color, background_strong_color, primary_color,
        style_metrics::ACCENT_STROKE_PX, text_color,
    },
};

const SLIDER_WIDTH_PX: f32 = 60.0;
const RESTING_KNOB_PER_GLYPH: f32 = 0.17;
const ENGAGED_KNOB_GROWTH: f32 = 1.25;

const DEFAULT_ICON_SIZE_PX: f32 = 14.0;
const DEFAULT_STEP: f32 = 1.0;

pub struct InlineSlider<'a, Message> {
    range: RangeInclusive<f32>,
    value: f32,
    on_change: Box<dyn Fn(f32) -> Message + 'a>,
    icon_size_px: f32,
    stroke_width_px: f32,
    lagging_value: Option<f32>,
    step: f32,
    accents: SliderAccents<'a>,
    on_release: Option<Message>,
    on_scroll: Option<Box<dyn Fn(ScrollDelta) -> Message + 'a>>,
}

pub fn inline_slider<'a, Message: Clone + 'a>(
    range: RangeInclusive<f32>,
    value: f32,
    on_change: impl Fn(f32) -> Message + 'a,
) -> InlineSlider<'a, Message> {
    InlineSlider {
        range,
        value,
        on_change: Box::new(on_change),
        icon_size_px: DEFAULT_ICON_SIZE_PX,
        stroke_width_px: ACCENT_STROKE_PX,
        lagging_value: None,
        step: DEFAULT_STEP,
        accents: SliderAccents::default(),
        on_release: None,
        on_scroll: None,
    }
}

impl<'a, Message> InlineSlider<'a, Message> {
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
    pub fn lagging_behind(self, lagging_value: Option<f32>) -> Self {
        Self {
            lagging_value,
            ..self
        }
    }

    #[must_use]
    pub fn step(self, step: f32) -> Self {
        Self { step, ..self }
    }

    #[must_use]
    pub fn accents(self, accents: SliderAccents<'a>) -> Self {
        Self { accents, ..self }
    }

    #[must_use]
    pub fn on_release(self, on_release: Message) -> Self {
        Self {
            on_release: Some(on_release),
            ..self
        }
    }

    #[must_use]
    pub fn on_scroll(self, on_scroll: impl Fn(ScrollDelta) -> Message + 'a) -> Self {
        Self {
            on_scroll: Some(Box::new(on_scroll)),
            ..self
        }
    }
}

impl<'a, Message: Clone + 'a> From<InlineSlider<'a, Message>> for Element<'a, Message> {
    fn from(inline: InlineSlider<'a, Message>) -> Self {
        let geometry = FaceGeometry::from_sizes(inline.icon_size_px, inline.stroke_width_px);
        let face = SliderFace {
            range: inline.range.clone(),
            value: inline.value,
            lagging_value: inline.lagging_value,
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
            inline.on_release,
        );
        let stacked = mouse_area(stack![interaction, Element::new(face)]);
        match inline.on_scroll {
            Some(on_scroll) => stacked.on_scroll(on_scroll).into(),
            None => stacked.into(),
        }
    }
}

fn interaction_slider<'a, Message: Clone + 'a>(
    geometry: FaceGeometry,
    range: RangeInclusive<f32>,
    value: f32,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
    on_release: Option<Message>,
) -> Slider<'a, f32, Message> {
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
    match on_release {
        Some(on_release) => styled.on_release(on_release),
        None => styled,
    }
}

#[derive(Debug, Clone, Copy)]
struct FaceGeometry {
    stroke_width: f32,
    resting_knob_radius: f32,
    engaged_knob_radius: f32,
}

impl FaceGeometry {
    fn from_sizes(icon_size_px: f32, stroke_width: f32) -> Self {
        let resting_knob_radius = icon_size_px * RESTING_KNOB_PER_GLYPH;
        Self {
            stroke_width,
            resting_knob_radius,
            engaged_knob_radius: resting_knob_radius * ENGAGED_KNOB_GROWTH,
        }
    }

    fn edge_inset(self) -> f32 {
        self.engaged_knob_radius + self.stroke_width / 2.0
    }
}

pub struct SliderAccents<'a> {
    pub resting: ThemedColor<'a>,
    pub engaged: ThemedColor<'a>,
}

impl Default for SliderAccents<'_> {
    fn default() -> Self {
        Self {
            resting: Box::new(text_color),
            engaged: Box::new(primary_color),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnobEngagement {
    Engaged,
    Resting,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PointerEngagement {
    hovered: bool,
    dragging: bool,
}

impl PointerEngagement {
    fn after_event(self, event: &Event, bounds: Rectangle, cursor: Cursor) -> Self {
        let hovered = cursor.is_over(bounds);
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => Self {
                hovered,
                dragging: hovered,
            },
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Self {
                hovered,
                dragging: false,
            },
            Event::Mouse(_) => Self { hovered, ..self },
            _ => self,
        }
    }

    fn knob_engagement(self) -> KnobEngagement {
        if self.hovered || self.dragging {
            KnobEngagement::Engaged
        } else {
            KnobEngagement::Resting
        }
    }
}

struct SliderFace<'a> {
    range: RangeInclusive<f32>,
    value: f32,
    lagging_value: Option<f32>,
    accents: SliderAccents<'a>,
    geometry: FaceGeometry,
    face_size: Size,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for SliderFace<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<PointerEngagement>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(PointerEngagement::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.face_size.width),
            Length::Fixed(self.face_size.height),
        )
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        Node::new(limits.resolve(self.face_size.width, self.face_size.height, Size::ZERO))
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let engagement = tree.state.downcast_mut::<PointerEngagement>();
        let next_engagement = engagement.after_event(event, layout.bounds(), cursor);
        if next_engagement != *engagement {
            *engagement = next_engagement;
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let engagement = tree.state.downcast_ref::<PointerEngagement>();
        let bounds = layout.bounds();
        let geometry = self.geometry;
        let (range_start, range_end) = (*self.range.start(), *self.range.end());
        let span = range_end - range_start;
        let edge_inset = geometry.edge_inset();
        let centre_of = |position: f32| {
            let fraction = if span > 0.0 {
                ((position - range_start) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            bounds.x + (bounds.width - 2.0 * edge_inset) * fraction + edge_inset
        };

        let knob_engagement = match self.lagging_value {
            Some(_) => KnobEngagement::Engaged,
            None => engagement.knob_engagement(),
        };
        let (accent, knob_radius) = match knob_engagement {
            KnobEngagement::Engaged => {
                ((self.accents.engaged)(theme), geometry.engaged_knob_radius)
            }
            KnobEngagement::Resting => {
                ((self.accents.resting)(theme), geometry.resting_knob_radius)
            }
        };

        let rail_y = bounds.y + bounds.height / 2.0;
        let handle_x = centre_of(self.value);
        let lagging_gap = self.lagging_value.map(|lagging| {
            let lagging_x = centre_of(lagging);
            (
                lagging_x - geometry.resting_knob_radius,
                lagging_x + geometry.resting_knob_radius,
            )
        });
        let accent_outline = QuadOutline {
            color: accent,
            width: geometry.stroke_width,
        };

        [
            (
                bounds.x + edge_inset,
                bounds.x + bounds.width - edge_inset,
                background_strong_color(theme),
            ),
            (bounds.x + edge_inset, handle_x, accent),
        ]
        .into_iter()
        .flat_map(|(segment_start, segment_end, color)| {
            without_gap(segment_start, segment_end, lagging_gap)
                .map(move |(piece_start, piece_end)| (piece_start, piece_end, color))
        })
        .for_each(|(piece_start, piece_end, color)| {
            HorizontalCapsuleQuad {
                start_x: piece_start,
                end_x: piece_end,
                centre_y: rail_y,
                thickness: geometry.stroke_width,
            }
            .draw(renderer, color);
        });

        if let Some(lagging_value) = self.lagging_value {
            CircleQuad::stroked_path(
                Point::new(centre_of(lagging_value), rail_y),
                geometry.resting_knob_radius,
                geometry.stroke_width,
            )
            .draw(renderer, Color::TRANSPARENT, accent_outline);
        }

        CircleQuad::stroked_path(
            Point::new(handle_x, rail_y),
            knob_radius,
            geometry.stroke_width,
        )
        .draw(renderer, background_color(theme), accent_outline);
    }
}

fn without_gap(
    segment_start: f32,
    segment_end: f32,
    gap: Option<(f32, f32)>,
) -> impl Iterator<Item = (f32, f32)> {
    let (gap_start, gap_end) = gap.unwrap_or((f32::INFINITY, f32::INFINITY));
    [
        (segment_start, segment_end.min(gap_start)),
        (segment_start.max(gap_end), segment_end),
    ]
    .into_iter()
    .filter(|(piece_start, piece_end)| piece_end > piece_start)
}
