use iced::{
    Border, Element, Length, Point, Rectangle, Size, Theme,
    advanced::{
        Layout, Renderer as _, Widget,
        layout::{Limits, Node},
        renderer::{self, Quad},
        widget::Tree,
    },
    border::Radius,
    mouse::Cursor,
};

use crate::{orientation::Axis, theming::ThemedColor};

pub struct ColorSegmentTrack {
    markers: Vec<ThemedColor<'static>>,
    segment_thickness: f32,
    segment_length: f32,
    axis: Axis,
}

impl ColorSegmentTrack {
    #[must_use]
    pub fn new(
        markers: Vec<ThemedColor<'static>>,
        segment_thickness: f32,
        segment_length: f32,
        axis: Axis,
    ) -> Self {
        Self {
            markers,
            segment_thickness,
            segment_length,
            axis,
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(self) -> Element<'a, Message> {
        Element::new(self)
    }

    fn track_size(&self) -> Size<Length> {
        let thickness = Length::Fixed(self.segment_thickness);
        match self.axis {
            Axis::Vertical => Size::new(thickness, Length::Fill),
            Axis::Horizontal => Size::new(Length::Fill, thickness),
        }
    }

    fn total_gap_length(&self) -> f32 {
        (self.markers.len() as f32 - 1.0) * self.segment_thickness
    }

    fn fitted_segment_length(&self, track_length: f32) -> f32 {
        let available_length = (track_length - self.total_gap_length()).max(0.0);
        self.segment_length
            .min(available_length / self.markers.len() as f32)
    }

    fn segment_starts(&self, track_length: f32) -> impl Iterator<Item = f32> {
        let fitted_length = self.fitted_segment_length(track_length);
        let stride = fitted_length + self.segment_thickness;
        let occupied_length = fitted_length * self.markers.len() as f32 + self.total_gap_length();
        let leading = (track_length - occupied_length) / 2.0;
        (0..self.markers.len()).map(move |position| leading + stride * position as f32)
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for ColorSegmentTrack {
    fn size(&self) -> Size<Length> {
        self.track_size()
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        let track_size = self.track_size();
        Node::new(limits.resolve(track_size.width, track_size.height, Size::ZERO))
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        if self.markers.is_empty() {
            return;
        }
        let bounds = layout.bounds();
        let track_length = match self.axis {
            Axis::Vertical => bounds.height,
            Axis::Horizontal => bounds.width,
        };
        let fitted_length = self.fitted_segment_length(track_length);
        let thickness = self.segment_thickness;
        self.segment_starts(track_length)
            .zip(&self.markers)
            .for_each(|(along, color)| {
                let segment_bounds = match self.axis {
                    Axis::Vertical => Rectangle::new(
                        Point::new(bounds.x, bounds.y + along),
                        Size::new(thickness, fitted_length),
                    ),
                    Axis::Horizontal => Rectangle::new(
                        Point::new(bounds.x + along, bounds.y),
                        Size::new(fitted_length, thickness),
                    ),
                };
                renderer.fill_quad(
                    Quad {
                        bounds: segment_bounds,
                        border: Border {
                            radius: Radius::new(thickness / 2.0),
                            ..Border::default()
                        },
                        ..Quad::default()
                    },
                    color(theme),
                );
            });
    }
}
