use std::f32::consts::{FRAC_PI_2, TAU};

use iced::{
    Color, Element, Length, Point, Radians, Rectangle, Renderer, Theme,
    mouse::Cursor,
    widget::{
        canvas,
        canvas::{LineCap, Stroke},
    },
};
use uom::si::{f64::Ratio, ratio::ratio};

const RING_THICKNESS_FRACTION: f32 = 0.12;

pub struct RefreshDial {
    elapsed_fraction: Ratio,
    diameter: f32,
    fill: Color,
    track: Color,
}

impl RefreshDial {
    #[must_use]
    pub fn new(elapsed_fraction: Ratio, diameter: f32, fill: Color, track: Color) -> Self {
        Self {
            elapsed_fraction,
            diameter,
            fill,
            track,
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(self) -> Element<'a, Message> {
        let diameter = self.diameter;
        canvas(self)
            .width(Length::Fixed(diameter))
            .height(Length::Fixed(diameter))
            .into()
    }

    fn ring(&self, color: Color) -> Stroke<'static> {
        Stroke::default()
            .with_color(color)
            .with_width(self.diameter * RING_THICKNESS_FRACTION)
            .with_line_cap(LineCap::Butt)
    }

    fn arc(center: Point, radius: f32, swept: f32) -> canvas::Path {
        canvas::Path::new(|builder| {
            builder.arc(canvas::path::Arc {
                center,
                radius,
                start_angle: Radians(-FRAC_PI_2),
                end_angle: Radians(-FRAC_PI_2 + swept),
            });
        })
    }
}

impl<Message> canvas::Program<Message> for RefreshDial {
    type State = ();

    #[allow(clippy::cast_possible_truncation)]
    fn draw(
        &self,
        (): &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let radius = self.diameter * (1.0 - RING_THICKNESS_FRACTION) / 2.0;
        frame.stroke(&canvas::Path::circle(center, radius), self.ring(self.track));
        let swept = TAU * self.elapsed_fraction.get::<ratio>() as f32;
        frame.stroke(&Self::arc(center, radius, swept), self.ring(self.fill));
        vec![frame.into_geometry()]
    }
}
