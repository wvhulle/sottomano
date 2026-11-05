use iced::{
    Color, Element, Length, Rectangle, Size, Theme,
    advanced::{
        Layout, Renderer as _, Widget,
        layout::{Limits, Node},
        renderer::{self, Quad},
        widget::Tree,
    },
    border,
    mouse::Cursor,
};

use crate::theme_colors::ThemedColor;

const DOT_DIAMETER_PER_LINE: f32 = 0.16;
const DOT_PITCH_PER_LINE: f32 = 0.5;

pub struct EllipsisRule<'a> {
    dot_color: ThemedColor<'a>,
    line_height_px: f32,
}

#[must_use]
pub fn ellipsis_rule<'a>(
    dot_color: impl Fn(&Theme) -> Color + 'a,
    line_height_px: f32,
) -> EllipsisRule<'a> {
    EllipsisRule {
        dot_color: Box::new(dot_color),
        line_height_px,
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for EllipsisRule<'_> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.line_height_px))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &Limits) -> Node {
        Node::new(limits.resolve(Length::Fill, Length::Fixed(self.line_height_px), Size::ZERO))
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
        let dot_color = (self.dot_color)(theme);
        let dot_diameter = self.line_height_px * DOT_DIAMETER_PER_LINE;
        let dot_pitch = self.line_height_px * DOT_PITCH_PER_LINE;
        let dot_count = ((bounds.width - dot_diameter) / dot_pitch).floor().max(0.0) as usize + 1;
        let run_width = dot_pitch * (dot_count - 1) as f32 + dot_diameter;
        let run_start = bounds.x + (bounds.width - run_width).max(0.0) / 2.0;
        let dot_top = bounds.y + (bounds.height - dot_diameter) / 2.0;
        (0..dot_count).for_each(|dot_index| {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: run_start + dot_pitch * dot_index as f32,
                        y: dot_top,
                        width: dot_diameter,
                        height: dot_diameter,
                    },
                    border: border::rounded(dot_diameter / 2.0),
                    ..Quad::default()
                },
                dot_color,
            );
        });
    }
}

impl<'a, Message: 'a> From<EllipsisRule<'a>> for Element<'a, Message> {
    fn from(rule: EllipsisRule<'a>) -> Self {
        Element::new(rule)
    }
}
