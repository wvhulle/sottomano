use core::slice;

use iced::{
    Border, Color, Element, Event, Length, Padding, Rectangle, Shadow, Size, Vector,
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
use sottomano_config::{Config, screen_edges::Position};
use sottomano_state::{LayerAppearance, ShellLayout};

use crate::widgets::{palette_colors::PaletteColors, region_reporter::rounded_blur_rows};

pub struct LayerFrameStyle<'a> {
    appearance: &'a LayerAppearance,
    config: &'a Config,
}

impl<'a> LayerFrameStyle<'a> {
    #[must_use]
    pub fn new(appearance: &'a LayerAppearance, config: &'a Config) -> Self {
        Self { appearance, config }
    }

    #[must_use]
    pub fn container_style(&self) -> container::Style {
        let colors = PaletteColors::from(&self.config.theme.palette);
        container::Style::default()
            .background(colors.surface.scale_alpha(self.appearance.opacity))
            .border(Border {
                radius: Radius::from(self.appearance.corner_radius_px),
                ..self.outline_border()
            })
            .shadow(self.drop_shadow())
            .color(colors.bar_foreground)
    }

    #[must_use]
    pub fn outline_border(&self) -> Border {
        if !self.appearance.outline.drawn {
            return Border::default();
        }
        Border {
            color: PaletteColors::from(&self.config.theme.palette).frame_outline,
            width: self.config.panel_bar.frame_outline,
            radius: Radius::default(),
        }
    }

    #[must_use]
    pub fn drop_shadow(&self) -> Shadow {
        let shadow = &self.appearance.shadow;
        if !shadow.cast {
            return Shadow::default();
        }
        Shadow {
            color: Color::BLACK.scale_alpha(shadow.opacity),
            offset: Vector::ZERO,
            blur_radius: shadow.softness_px,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct OutlinedSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl OutlinedSides {
    pub const ALL: Self = Self {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };

    #[must_use]
    pub fn inward_of(panel_position: Position) -> Self {
        Self {
            top: panel_position != Position::Top,
            right: true,
            bottom: panel_position != Position::Bottom,
            left: panel_position != Position::Left,
        }
    }

    #[must_use]
    pub fn all_when_floating(self, layout: &ShellLayout) -> Self {
        if layout.float_offset_px > 0.0 {
            Self::ALL
        } else {
            self
        }
    }

    fn square_unoutlined_corners(self, radius: Radius) -> Radius {
        let rounded_between = |first: bool, second: bool, corner: f32| {
            if first && second { corner } else { 0.0 }
        };
        Radius {
            top_left: rounded_between(self.top, self.left, radius.top_left),
            top_right: rounded_between(self.top, self.right, radius.top_right),
            bottom_right: rounded_between(self.bottom, self.right, radius.bottom_right),
            bottom_left: rounded_between(self.bottom, self.left, radius.bottom_left),
        }
    }

    fn push_unoutlined_past(self, bounds: Rectangle, outline_width: f32) -> Rectangle {
        let beyond_antialiasing = |outlined: bool| if outlined { 0.0 } else { 2.0 * outline_width };
        bounds.expand(Padding {
            top: beyond_antialiasing(self.top),
            right: beyond_antialiasing(self.right),
            bottom: beyond_antialiasing(self.bottom),
            left: beyond_antialiasing(self.left),
        })
    }
}

pub fn panel_frame<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    style: &container::Style,
) -> PanelFrame<'a, Message> {
    PanelFrame {
        content: content.into(),
        style: *style,
        outlined_sides: OutlinedSides::ALL,
        blurred: false,
    }
}

pub struct PanelFrame<'a, Message> {
    content: Element<'a, Message>,
    style: container::Style,
    outlined_sides: OutlinedSides,
    blurred: bool,
}

impl<Message> PanelFrame<'_, Message> {
    #[must_use]
    pub fn outlined_sides(mut self, sides: OutlinedSides) -> Self {
        self.outlined_sides = sides;
        self
    }

    #[must_use]
    pub fn blurred(mut self, blurred: bool) -> Self {
        self.blurred = blurred;
        self
    }

    fn drawn_radius(&self) -> Radius {
        self.outlined_sides
            .square_unoutlined_corners(self.style.border.radius)
    }

    fn blur_regions(&self, bounds: Rectangle) -> BlurRegions {
        if !self.blurred {
            return BlurRegions::default();
        }
        BlurRegions(rounded_blur_rows(bounds, self.drawn_radius()))
    }

    fn draw_fill(&self, renderer: &mut iced::Renderer, bounds: Rectangle, radius: Radius) {
        let fill_without_outline = container::Style {
            border: Border {
                width: 0.0,
                radius,
                ..self.style.border
            },
            ..self.style
        };
        container::draw_background(renderer, &fill_without_outline, bounds);
    }

    fn draw_outline(&self, renderer: &mut iced::Renderer, bounds: Rectangle, radius: Radius) {
        let outline = self.style.border;
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
        self.draw_fill(renderer, bounds, radius);
        self.draw_outline(renderer, bounds, radius);
        let framed_style = renderer::Style {
            text_color: self.style.text_color.unwrap_or(style.text_color),
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
    use iced::{
        Rectangle,
        border::{Border, Radius},
        widget::{container, space},
    };
    use sottomano_config::screen_edges::Position;

    use super::{OutlinedSides, panel_frame};

    #[test]
    fn a_top_panel_blurs_its_screen_edge_corners_square() {
        let style = container::Style::default().border(Border {
            radius: Radius::from(8.0),
            ..Border::default()
        });
        let frame = panel_frame::<()>(space(), &style)
            .outlined_sides(OutlinedSides::inward_of(Position::Top))
            .blurred(true);
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
