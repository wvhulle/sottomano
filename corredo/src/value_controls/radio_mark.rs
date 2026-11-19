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

use super::toggle_switch::{SwitchResponse, TogglePosition, knob_color, track_color};
use crate::quad_shapes::{CircleQuad, QuadOutline};

const DEFAULT_MARK_SIZE_PX: f32 = 16.0;
const DOT_FRACTION_OF_RADIUS: f32 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioMark {
    toggle_position: TogglePosition,
    response: SwitchResponse,
    size_px: f32,
}

#[must_use]
pub fn radio_mark(toggle_position: TogglePosition) -> RadioMark {
    RadioMark {
        toggle_position,
        response: SwitchResponse::Pressable,
        size_px: DEFAULT_MARK_SIZE_PX,
    }
}

impl RadioMark {
    #[must_use]
    pub fn size(self, size_px: f32) -> Self {
        Self { size_px, ..self }
    }

    #[must_use]
    pub fn response(self, response: SwitchResponse) -> Self {
        Self { response, ..self }
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for RadioMark {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size_px), Length::Fixed(self.size_px))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        Node::new(limits.resolve(
            Length::Fixed(self.size_px),
            Length::Fixed(self.size_px),
            Size::ZERO,
        ))
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
        let bounds = layout.bounds();
        let radius = bounds.width.min(bounds.height) / 2.0;
        let centre = Point::new(bounds.center_x(), bounds.center_y());
        CircleQuad { centre, radius }.draw(
            renderer,
            track_color(theme, self.toggle_position, self.response),
            QuadOutline::NONE,
        );
        if self.toggle_position == TogglePosition::On {
            CircleQuad {
                centre,
                radius: radius * DOT_FRACTION_OF_RADIUS,
            }
            .draw(
                renderer,
                knob_color(theme, self.response),
                QuadOutline::NONE,
            );
        }
    }
}

impl<'a, Message: 'a> From<RadioMark> for Element<'a, Message> {
    fn from(mark: RadioMark) -> Self {
        Element::new(mark)
    }
}

#[cfg(test)]
mod tests {
    use iced::{Settings, widget::row};
    use iced_test::Simulator;

    use super::*;

    #[test]
    fn a_mark_takes_the_requested_square_size_in_either_position() {
        [TogglePosition::On, TogglePosition::Off]
            .into_iter()
            .for_each(|toggle_position| {
                let mark = radio_mark(toggle_position).size(12.0);
                assert_eq!(
                    Widget::<(), Theme, iced::Renderer>::size(&mark),
                    Size::new(Length::Fixed(12.0), Length::Fixed(12.0))
                );
                let marks_row: Element<'_, ()> = row![mark].into();
                let _rendered =
                    Simulator::with_size(Settings::default(), Size::new(40.0, 40.0), marks_row);
            });
    }
}
