use iced::{
    Border, Color, Point, Rectangle, Size,
    advanced::{Renderer as _, renderer::Quad},
    border::Radius,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct QuadOutline {
    pub color: Color,
    pub width: f32,
}

impl QuadOutline {
    pub(crate) const NONE: Self = Self {
        color: Color::TRANSPARENT,
        width: 0.0,
    };
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CircleQuad {
    pub centre: Point,
    pub radius: f32,
}

impl CircleQuad {
    #[must_use]
    pub(crate) fn stroked_path(centre: Point, path_radius: f32, stroke_width: f32) -> Self {
        Self {
            centre,
            radius: path_radius + stroke_width / 2.0,
        }
    }

    pub(crate) fn draw(self, renderer: &mut iced::Renderer, fill: Color, outline: QuadOutline) {
        let diameter = 2.0 * self.radius;
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    Point::new(self.centre.x - self.radius, self.centre.y - self.radius),
                    Size::new(diameter, diameter),
                ),
                border: Border {
                    color: outline.color,
                    width: outline.width,
                    radius: Radius::new(self.radius),
                },
                ..Quad::default()
            },
            fill,
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HorizontalCapsuleQuad {
    pub start_x: f32,
    pub end_x: f32,
    pub centre_y: f32,
    pub thickness: f32,
}

impl HorizontalCapsuleQuad {
    pub(crate) fn draw(self, renderer: &mut iced::Renderer, fill: Color) {
        let half_thickness = self.thickness / 2.0;
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::new(
                    Point::new(
                        self.start_x - half_thickness,
                        self.centre_y - half_thickness,
                    ),
                    Size::new(self.end_x - self.start_x + self.thickness, self.thickness),
                ),
                border: Border {
                    radius: Radius::new(half_thickness),
                    ..Border::default()
                },
                ..Quad::default()
            },
            fill,
        );
    }
}
