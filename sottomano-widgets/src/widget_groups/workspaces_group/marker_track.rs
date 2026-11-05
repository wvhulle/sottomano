use iced::{
    Element, Length, Point, Rectangle, Size, Theme,
    advanced::{
        Layout, Widget,
        layout::{Limits, Node},
        renderer,
        widget::Tree,
    },
    mouse::Cursor,
};
use sottomano_toml_config::screen_edges::Position;

use crate::widgets::{
    configured_theme::ThemedColor,
    rounded_quads::{CircleQuad, QuadOutline},
};

pub struct MarkerTrack {
    markers: Vec<ThemedColor<'static>>,
    max_diameter: f32,
    along_vertical_edge: bool,
}

impl MarkerTrack {
    #[must_use]
    pub fn new(markers: Vec<ThemedColor<'static>>, max_diameter: f32, edge: Position) -> Self {
        Self {
            markers,
            max_diameter,
            along_vertical_edge: edge.is_vertical(),
        }
    }

    #[must_use]
    pub fn view<'a, Message: 'a>(self) -> Element<'a, Message> {
        Element::new(self)
    }

    fn track_size(&self) -> Size<Length> {
        let thickness = Length::Fixed(self.max_diameter);
        if self.along_vertical_edge {
            Size::new(thickness, Length::Fill)
        } else {
            Size::new(Length::Fill, thickness)
        }
    }

    fn marker_and_gap_slots(&self) -> f32 {
        self.markers.len() as f32 * 2.0 - 1.0
    }

    fn marker_diameter(&self, track_length: f32) -> f32 {
        self.max_diameter
            .min(track_length / self.marker_and_gap_slots())
    }

    fn marker_centres(&self, track_length: f32) -> impl Iterator<Item = f32> {
        let diameter = self.marker_diameter(track_length);
        let leading = (track_length - self.marker_and_gap_slots() * diameter) / 2.0;
        (0..self.markers.len()).map(move |i| leading + diameter * (2.0 * i as f32 + 0.5))
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for MarkerTrack {
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
        let (track_length, cross) = if self.along_vertical_edge {
            (bounds.height, bounds.width / 2.0)
        } else {
            (bounds.width, bounds.height / 2.0)
        };
        let radius = self.marker_diameter(track_length) / 2.0;
        self.marker_centres(track_length)
            .zip(&self.markers)
            .for_each(|(along, color)| {
                let offset_centre = if self.along_vertical_edge {
                    Point::new(cross, along)
                } else {
                    Point::new(along, cross)
                };
                CircleQuad {
                    centre: offset_centre + (bounds.position() - Point::ORIGIN),
                    radius,
                }
                .draw(renderer, color(theme), QuadOutline::NONE);
            });
    }
}
