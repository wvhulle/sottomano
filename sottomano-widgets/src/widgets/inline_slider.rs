use std::ops::RangeInclusive;

use iced::{
    Color, Element, Event, Point, Rectangle, Renderer, Theme,
    mouse::{self, Cursor, ScrollDelta},
    widget::{
        Slider,
        canvas::{self, LineCap, Path, Stroke},
        mouse_area, slider, stack,
    },
};
use sottomano_config::{Config, severity::Severity};

use crate::widgets::{
    configured_theme::{background_color, background_strong_color, primary_color, text_color},
    widget_state::WidgetState,
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
    };
    mouse_area(stack![
        interaction_slider(config, geometry, range, value, step, on_change, on_release),
        canvas::Canvas::new(face)
            .width(config.panel_bar.slider_width)
            .height(Slider::<f32, Message>::DEFAULT_HEIGHT)
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
            other => WidgetState::from(other).color(theme),
        }
    }

    fn engaged(self, theme: &Theme) -> Color {
        match self.level {
            Severity::Normal => primary_color(theme),
            other => WidgetState::from(other).color(theme),
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
}

impl<Message> canvas::Program<Message> for SliderFace {
    type State = PointerEngagement;

    fn update(
        &self,
        engagement: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Option<canvas::Action<Message>> {
        let next_engagement = engagement.after_event(event, bounds, cursor);
        (next_engagement != *engagement).then(|| {
            *engagement = next_engagement;
            canvas::Action::request_redraw()
        })
    }

    fn draw(
        &self,
        engagement: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
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
            (bounds.width - 2.0 * edge_inset) * fraction + edge_inset
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

        let rail_y = bounds.height / 2.0;
        let handle_x = centre_of(self.value);
        let lagging_gap = self.lagging_value.map(|lagging| {
            let lagging_x = centre_of(lagging);
            (
                lagging_x - geometry.resting_knob_radius,
                lagging_x + geometry.resting_knob_radius,
            )
        });
        let line_stroke = |color: Color| {
            Stroke::default()
                .with_width(geometry.stroke_width)
                .with_color(color)
                .with_line_cap(LineCap::Round)
        };

        let mut frame = canvas::Frame::new(renderer, bounds.size());

        [
            (
                edge_inset,
                bounds.width - edge_inset,
                background_strong_color(theme),
            ),
            (edge_inset, handle_x, accent),
        ]
        .into_iter()
        .flat_map(|(segment_start, segment_end, color)| {
            without_gap(segment_start, segment_end, lagging_gap)
                .map(move |(piece_start, piece_end)| (piece_start, piece_end, color))
        })
        .for_each(|(piece_start, piece_end, color)| {
            frame.stroke(
                &Path::line(
                    Point::new(piece_start, rail_y),
                    Point::new(piece_end, rail_y),
                ),
                line_stroke(color),
            );
        });

        if let Some(lagging_value) = self.lagging_value {
            frame.stroke(
                &Path::circle(
                    Point::new(centre_of(lagging_value), rail_y),
                    geometry.resting_knob_radius,
                ),
                line_stroke(accent),
            );
        }

        let knob = Path::circle(Point::new(handle_x, rail_y), knob_radius);
        frame.fill(&knob, background_color(theme));
        frame.stroke(&knob, line_stroke(accent));

        vec![frame.into_geometry()]
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
