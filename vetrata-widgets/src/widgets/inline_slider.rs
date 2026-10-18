use std::ops::RangeInclusive;

use iced::{
    Color, Element, Point, Rectangle, Renderer, Size, Theme,
    mouse::{Cursor, ScrollDelta},
    widget::{Slider, canvas, mouse_area, slider, stack},
};
use vetrata_config::{Config, severity::Severity};

use crate::widgets::{palette_colors::PaletteColors, widget_state::WidgetState};

const KNOB_RADIUS: f32 = 5.0;
const KNOB_RADIUS_DRAG: f32 = 7.0;
const EDGE_INSET: f32 = KNOB_RADIUS_DRAG;
const RAIL_WIDTH: f32 = 4.0;

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
    let accent = match level {
        Severity::Normal => PaletteColors::from(&config.theme.palette).focus,
        _ => WidgetState::from(level).color(&PaletteColors::from(&config.theme.palette)),
    };
    let face = SliderFace {
        range: range.clone(),
        value,
        lagging_value,
        accent,
        track: PaletteColors::from(&config.theme.palette).surface_raised,
    };
    mouse_area(stack![
        interaction_slider(config, range, value, step, on_change, on_release),
        canvas(face)
            .width(config.panel_bar.slider_width)
            .height(Slider::<f32, Message>::DEFAULT_HEIGHT)
    ])
    .on_scroll(on_scroll)
    .into()
}

fn interaction_slider<'a, Message: Clone + 'a>(
    config: &Config,
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
        .style(|_theme, _status| slider::Style {
            rail: slider::Rail {
                backgrounds: (Color::TRANSPARENT.into(), Color::TRANSPARENT.into()),
                width: RAIL_WIDTH,
                border: iced::Border::default(),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle {
                    radius: KNOB_RADIUS,
                },
                background: Color::TRANSPARENT.into(),
                border_color: Color::TRANSPARENT,
                border_width: 0.0,
            },
        })
}

struct SliderFace {
    range: RangeInclusive<f32>,
    value: f32,
    lagging_value: Option<f32>,
    accent: Color,
    track: Color,
}

impl<Message> canvas::Program<Message> for SliderFace {
    type State = ();

    fn draw(
        &self,
        (): &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let (start, end) = (*self.range.start(), *self.range.end());
        let span = end - start;
        let centre = |v: f32| {
            let fraction = if span > 0.0 {
                ((v - start) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (bounds.width - 2.0 * EDGE_INSET) * fraction + EDGE_INSET
        };

        let rail_y = bounds.height / 2.0;
        let handle_x = centre(self.value);
        let gap = self
            .lagging_value
            .map(|at| (centre(at) - KNOB_RADIUS, centre(at) + KNOB_RADIUS));

        let mut frame = canvas::Frame::new(renderer, bounds.size());

        [
            (0.0, handle_x, self.accent),
            (handle_x, bounds.width, self.track),
        ]
        .into_iter()
        .flat_map(|(from, to, color)| without_gap(from, to, gap).map(move |(a, b)| (a, b, color)))
        .for_each(|(a, b, color)| {
            frame.fill_rectangle(
                Point::new(a, rail_y - RAIL_WIDTH / 2.0),
                Size::new(b - a, RAIL_WIDTH),
                color,
            );
        });

        if let Some(lagging_value) = self.lagging_value {
            frame.stroke(
                &canvas::Path::circle(Point::new(centre(lagging_value), rail_y), KNOB_RADIUS),
                canvas::Stroke::default()
                    .with_width(1.5)
                    .with_color(self.accent),
            );
        }

        let handle_radius = if self.lagging_value.is_some() {
            KNOB_RADIUS_DRAG
        } else {
            KNOB_RADIUS
        };
        frame.fill(
            &canvas::Path::circle(Point::new(handle_x, rail_y), handle_radius),
            self.accent,
        );

        vec![frame.into_geometry()]
    }
}

fn without_gap(from: f32, to: f32, gap: Option<(f32, f32)>) -> impl Iterator<Item = (f32, f32)> {
    let (g0, g1) = gap.unwrap_or((f32::INFINITY, f32::INFINITY));
    [(from, to.min(g0)), (from.max(g1), to)]
        .into_iter()
        .filter(|(a, b)| b > a)
}
