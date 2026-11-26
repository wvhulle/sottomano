use iced::{
    Element, Length, Point, Rectangle, Size,
    advanced::{
        Layout, Widget,
        image::{Image, Renderer as ImageRenderer},
        layout::{Limits, Node},
        renderer,
        widget::Tree,
    },
    mouse::Cursor,
};
use iced_exwlshell::SurfaceRegionsCollector;

use crate::backdrop_photo::BackdropPhotos;

#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn frosted_blur_rectangles(surface_regions: &SurfaceRegionsCollector) -> Vec<Rectangle> {
    surface_regions
        .blur
        .iter()
        .filter(|blur_row| blur_row.width > 0 && blur_row.height > 0)
        .map(|blur_row| Rectangle {
            x: blur_row.x as f32,
            y: blur_row.y as f32,
            width: blur_row.width as f32,
            height: blur_row.height as f32,
        })
        .collect()
}

fn covering_photo_bounds(photo_size: Size, canvas: Rectangle) -> Rectangle {
    let scale = (canvas.width / photo_size.width).max(canvas.height / photo_size.height);
    let covered_size = photo_size * scale;
    Rectangle::new(
        Point::new(
            canvas.center_x() - covered_size.width / 2.0,
            canvas.center_y() - covered_size.height / 2.0,
        ),
        covered_size,
    )
}

pub struct FrostedBackdrop<'view> {
    photos: &'view BackdropPhotos,
    frosted_rows: &'view [Rectangle],
}

impl<'view> FrostedBackdrop<'view> {
    #[must_use]
    pub fn new(photos: &'view BackdropPhotos, frosted_rows: &'view [Rectangle]) -> Self {
        FrostedBackdrop {
            photos,
            frosted_rows,
        }
    }
}

impl<Message, Theme> Widget<Message, Theme, iced::Renderer> for FrostedBackdrop<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        Node::new(limits.max())
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let canvas = layout.bounds();
        let covered = covering_photo_bounds(self.photos.photo_size, canvas);
        renderer.draw_image(Image::new(self.photos.sharp_photo.clone()), covered, canvas);
        self.frosted_rows
            .iter()
            .filter_map(|frosted_row| frosted_row.intersection(&canvas))
            .for_each(|clipped_row| {
                renderer.draw_image(
                    Image::new(self.photos.frosted_photo.clone()),
                    covered,
                    clipped_row,
                );
            });
    }
}

impl<'view, Message: 'view, Theme: 'view> From<FrostedBackdrop<'view>>
    for Element<'view, Message, Theme>
{
    fn from(backdrop: FrostedBackdrop<'view>) -> Self {
        Element::new(backdrop)
    }
}
