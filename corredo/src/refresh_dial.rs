use std::{
    f32::consts::{FRAC_PI_2, TAU},
    time::{Duration, Instant},
};

use iced::{
    Border, Color, Element, Event, Length, Point, Radians, Rectangle, Renderer, Theme,
    mouse::Cursor,
    widget::{
        Action, canvas,
        canvas::{LineCap, Stroke},
        container, space, stack,
    },
    window,
};

use crate::theme_colors::{background_strong_color, secondary_text_color};

const RING_THICKNESS_FRACTION: f32 = 0.12;

pub struct RefreshDial {
    elapsed: Duration,
    period: Duration,
    viewed_at: Instant,
    diameter: f32,
}

impl RefreshDial {
    #[must_use]
    pub fn new(elapsed: Duration, period: Duration, viewed_at: Instant, diameter: f32) -> Self {
        Self {
            elapsed,
            period,
            viewed_at,
            diameter,
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(self) -> Element<'a, Message> {
        let diameter = self.diameter;
        let ring_width = self.ring_width();
        stack![
            container(space())
                .width(Length::Fixed(diameter))
                .height(Length::Fixed(diameter))
                .style(move |theme| container::Style::default().border(Border {
                    color: background_strong_color(theme),
                    width: ring_width,
                    radius: (diameter / 2.0).into(),
                })),
            canvas(self)
                .width(Length::Fixed(diameter))
                .height(Length::Fixed(diameter))
        ]
        .into()
    }

    fn ring_width(&self) -> f32 {
        self.diameter * RING_THICKNESS_FRACTION
    }

    fn elapsed_fraction(&self, frame: Option<Instant>) -> f32 {
        let since_view = frame.map_or(Duration::ZERO, |frame| {
            frame.saturating_duration_since(self.viewed_at)
        });
        let elapsed = self.elapsed + since_view;
        (elapsed.as_secs_f32() / self.period.as_secs_f32().max(f32::EPSILON)).min(1.0)
    }

    fn ring(&self, color: Color) -> Stroke<'static> {
        Stroke::default()
            .with_color(color)
            .with_width(self.ring_width())
            .with_line_cap(LineCap::Butt)
    }
}

fn arc_path(center: Point, radius: f32, swept: f32) -> canvas::Path {
    canvas::Path::new(|builder| {
        builder.arc(canvas::path::Arc {
            center,
            radius,
            start_angle: Radians(-FRAC_PI_2),
            end_angle: Radians(-FRAC_PI_2 + swept),
        });
    })
}

impl<Message> canvas::Program<Message> for RefreshDial {
    type State = Option<Instant>;

    fn update(
        &self,
        frame: &mut Self::State,
        event: &Event,
        _bounds: Rectangle,
        _cursor: Cursor,
    ) -> Option<Action<Message>> {
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return None;
        };
        *frame = Some(*now);
        (self.elapsed_fraction(*frame) < 1.0).then(Action::request_redraw)
    }

    fn draw(
        &self,
        frame: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut geometry = canvas::Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = self.diameter * (1.0 - RING_THICKNESS_FRACTION) / 2.0;
        let swept = TAU * self.elapsed_fraction(*frame);
        geometry.stroke(
            &arc_path(center, radius, swept),
            self.ring(secondary_text_color(theme)),
        );
        vec![geometry.into_geometry()]
    }
}
