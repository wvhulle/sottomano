use iced::{
    Color, Element, Length, Rectangle, Size,
    advanced::{
        Layout, Renderer as _, Widget,
        layout::{Limits, Node},
        renderer::{self, Quad},
        widget::Tree,
    },
    border,
    mouse::Cursor,
};

const DOT_DIAMETER_PER_LINE: f32 = 0.16;
const DOT_PITCH_PER_LINE: f32 = 0.5;

pub struct EllipsisRule {
    dot_color: Color,
    line_height_px: f32,
}

#[must_use]
pub fn ellipsis_rule(dot_color: Color, line_height_px: f32) -> EllipsisRule {
    EllipsisRule {
        dot_color,
        line_height_px,
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for EllipsisRule {
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
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
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
                self.dot_color,
            );
        });
    }
}

impl<'a, Message: 'a> From<EllipsisRule> for Element<'a, Message> {
    fn from(rule: EllipsisRule) -> Self {
        Element::new(rule)
    }
}
