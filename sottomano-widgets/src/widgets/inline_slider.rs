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
use sottomano_config::{Config, severity::Severity};

use crate::widgets::{
    configured_theme::{background_color, background_strong_color, primary_color, text_color},
    reading_display_state::ReadingDisplayState,
    rounded_quads::{CircleQuad, HorizontalCapsuleQuad, QuadOutline},
};

const RESTING_KNOB_PER_GLYPH: f32 = 0.17;
const ENGAGED_KNOB_GROWTH: f32 = 1.25;

#[allow(clippy::too_many_arguments)]
pub fn inline_slider<'a, Message: Clone + 'a>(
    config: &Config,
    range: RangeInclusive<f32>,
    value: f32,
    lagging_value: Option<f32>,
    step: f32,
    level: Severity,
    on_change: impl Fn(f32) -> Message + 'a,
    on_release: Message,
    on_scroll: impl Fn(ScrollDelta) -> Message + 'a,
) -> Element<'a, Message> {
    let accents = InteractionAccents { level };
    let geometry = FaceGeometry::from_sizes(
        config.theme.font.icon_size_px,
        config.panel_bar.slider_stroke,
    );
    let face = SliderFace {
        range: range.clone(),
        value,
        lagging_value,
        accents,
        geometry,
        face_size: Size::new(
            config.panel_bar.slider_width,
            Slider::<f32, Message>::DEFAULT_HEIGHT,
        ),
    };
    mouse_area(stack![
        interaction_slider(config, geometry, range, value, step, on_change, on_release),
        Element::new(face)
    ])
    .on_scroll(on_scroll)
    .into()
}

fn interaction_slider<'a, Message: Clone + 'a>(
    config: &Config,
    geometry: FaceGeometry,
    range: RangeInclusive<f32>,
    value: f32,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
    on_release: Message,
) -> Slider<'a, f32, Message> {
    slider(range, value, on_change)
        .step(step)
        .on_release(on_release)
        .width(config.panel_bar.slider_width)
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
        })
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

#[derive(Debug, Clone, Copy)]
struct InteractionAccents {
    level: Severity,
}

impl InteractionAccents {
    fn resting(self, theme: &Theme) -> Color {
        match self.level {
            Severity::Normal => text_color(theme),
            other => ReadingDisplayState::from(other).color(theme),
        }
    }

    fn engaged(self, theme: &Theme) -> Color {
        match self.level {
            Severity::Normal => primary_color(theme),
            other => ReadingDisplayState::from(other).color(theme),
        }
    }
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

    fn engaged(self) -> bool {
        self.hovered || self.dragging
    }
}

struct SliderFace {
    range: RangeInclusive<f32>,
    value: f32,
    lagging_value: Option<f32>,
    accents: InteractionAccents,
    geometry: FaceGeometry,
    face_size: Size,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for SliderFace {
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

        let engaged = engagement.engaged() || self.lagging_value.is_some();
        let accent = if engaged {
            self.accents.engaged(theme)
        } else {
            self.accents.resting(theme)
        };
        let knob_radius = if engaged {
            geometry.engaged_knob_radius
        } else {
            geometry.resting_knob_radius
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
