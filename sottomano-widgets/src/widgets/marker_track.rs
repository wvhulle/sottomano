use iced::{Element, Length, Point, Rectangle, Renderer, Theme, mouse::Cursor, widget::canvas};
use sottomano_config::screen_edges::Position;

use crate::widgets::configured_theme::ThemedColor;

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
        let thickness = Length::Fixed(self.max_diameter);
        let (width, height) = if self.along_vertical_edge {
            (thickness, Length::Fill)
        } else {
            (Length::Fill, thickness)
        };
        canvas(self).width(width).height(height).into()
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

impl<Message> canvas::Program<Message> for MarkerTrack {
    type State = ();

    fn draw(
        &self,
        (): &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if self.markers.is_empty() {
            return vec![frame.into_geometry()];
        }
        let (track_length, cross) = if self.along_vertical_edge {
            (bounds.height, bounds.width / 2.0)
        } else {
            (bounds.width, bounds.height / 2.0)
        };
        let radius = self.marker_diameter(track_length) / 2.0;
        self.marker_centres(track_length)
            .zip(&self.markers)
            .for_each(|(along, color)| {
                let centre = if self.along_vertical_edge {
                    Point::new(cross, along)
                } else {
                    Point::new(along, cross)
                };
                frame.fill(&canvas::Path::circle(centre, radius), color(theme));
            });
        vec![frame.into_geometry()]
    }
}
