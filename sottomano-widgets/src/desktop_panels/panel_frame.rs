use core::slice;

pub use corredo::frame_style::{FrameStyle, OutlinedSides};
use iced::{
    Border, Color, Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree},
    },
    border::Radius,
    mouse::Cursor,
    widget::container,
};
use iced_exwlshell::BlurRegions;
use sottomano_toml_config::screen_edges::Position;

use crate::desktop_panels::region_reporter::{
    BackdropBlur, replacing_blur_regions, rounded_blur_rows,
};

#[must_use]
pub fn outlined_sides_inward_of(panel_position: Position) -> OutlinedSides {
    OutlinedSides {
        top: panel_position != Position::Top,
        right: true,
        bottom: panel_position != Position::Bottom,
        left: panel_position != Position::Left,
    }
}

pub fn panel_frame<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    frame: FrameStyle,
) -> PanelFrame<'a, Message> {
    PanelFrame {
        content: content.into(),
        frame,
        outlined_sides: OutlinedSides::ALL,
        backdrop_blur: BackdropBlur::Clear,
    }
}

pub struct PanelFrame<'a, Message> {
    content: Element<'a, Message>,
    frame: FrameStyle,
    outlined_sides: OutlinedSides,
    backdrop_blur: BackdropBlur,
}

impl<Message> PanelFrame<'_, Message> {
    #[must_use]
    pub fn outlined_sides(mut self, sides: OutlinedSides) -> Self {
        self.outlined_sides = sides;
        self
    }

    #[must_use]
    pub fn backdrop_blur(mut self, backdrop_blur: BackdropBlur) -> Self {
        self.backdrop_blur = backdrop_blur;
        self
    }

    fn drawn_radius(&self) -> Radius {
        self.frame.drawn_corners(self.outlined_sides)
    }

    fn blur_regions(&self, bounds: Rectangle) -> BlurRegions {
        match self.backdrop_blur {
            BackdropBlur::Blurred => {
                replacing_blur_regions(rounded_blur_rows(bounds, self.drawn_radius()))
            }
            BackdropBlur::Clear => replacing_blur_regions(Vec::new()),
        }
    }

    fn draw_fill(
        renderer: &mut iced::Renderer,
        frame_style: &container::Style,
        bounds: Rectangle,
        radius: Radius,
    ) {
        let fill_without_outline = container::Style {
            border: Border {
                width: 0.0,
                radius,
                ..frame_style.border
            },
            ..*frame_style
        };
        container::draw_background(renderer, &fill_without_outline, bounds);
    }

    fn draw_outline(
        &self,
        renderer: &mut iced::Renderer,
        outline: Border,
        bounds: Rectangle,
        radius: Radius,
    ) {
        if outline.width <= 0.0 {
            return;
        }
        renderer.with_layer(bounds, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: self
                        .outlined_sides
                        .push_unoutlined_past(bounds, outline.width),
                    border: Border { radius, ..outline },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        });
    }
}

impl<'a, Message: 'a> From<PanelFrame<'a, Message>> for Element<'a, Message> {
    fn from(frame: PanelFrame<'a, Message>) -> Self {
        Self::new(frame)
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for PanelFrame<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let radius = self.drawn_radius();
        let resolved_frame = self.frame.container_style(theme);
        Self::draw_fill(renderer, &resolved_frame, bounds, radius);
        self.draw_outline(renderer, resolved_frame.border, bounds, radius);
        let framed_style = renderer::Style {
            text_color: resolved_frame.text_color.unwrap_or(style.text_color),
        };
        renderer.with_layer(*viewport, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &framed_style,
                layout,
                cursor,
                viewport,
            );
        });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let bounds = layout.bounds();
        operation.custom(None, bounds, &mut self.blur_regions(bounds));
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use iced::{Rectangle, widget::space};
    use sottomano_toml_config::screen_edges::Position;

    use super::{outlined_sides_inward_of, panel_frame};
    use crate::{
        desktop_panels::region_reporter::BackdropBlur,
        widgets::shell_layer_appearance::LayerAppearance,
    };

    #[test]
    fn a_top_panel_blurs_its_screen_edge_corners_square() {
        let appearance = LayerAppearance {
            corner_radius_px: 8.0,
            ..LayerAppearance::default()
        };
        let frame = panel_frame::<()>(space(), appearance.frame_style())
            .outlined_sides(outlined_sides_inward_of(Position::Top))
            .backdrop_blur(BackdropBlur::Blurred);
        let pill = Rectangle {
            x: 10.0,
            y: 0.0,
            width: 60.0,
            height: 24.0,
        };
        let rows = frame.blur_regions(pill).0;
        let top_row = rows.iter().find(|row| row.y == 0).expect("top row");
        let bottom_row = rows.iter().find(|row| row.y == 23).expect("bottom row");
        assert_eq!((top_row.x, top_row.width), (10, 60));
        assert!(bottom_row.x > 10 && bottom_row.width < 60);
    }
}
