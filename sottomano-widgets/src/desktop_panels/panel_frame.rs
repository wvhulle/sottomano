use core::slice;

use iced::{
    Border, Color, Element, Event, Length, Padding, Rectangle, Shadow, Size, Theme, Vector,
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
use sottomano_config::screen_edges::Position;

use crate::{
    desktop_panels::region_reporter::{replacing_blur_regions, rounded_blur_rows},
    widgets::{
        configured_theme::{
            background_color, background_strong_color, background_weak_color, text_color,
        },
        shared_metrics::OUTLINE_WIDTH_PX,
        shell_layer_appearance::{LayerAppearance, LayerBackground},
    },
};

const SHADOW_DROP_PER_SOFTNESS: f32 = 0.25;

#[derive(Debug, Clone, Copy)]
pub struct LayerFrameStyle {
    appearance: LayerAppearance,
    outline_width: f32,
}

impl LayerFrameStyle {
    #[must_use]
    pub fn new(appearance: &LayerAppearance) -> Self {
        Self {
            appearance: *appearance,
            outline_width: OUTLINE_WIDTH_PX,
        }
    }

    #[must_use]
    pub fn with_corner_radius(mut self, corner_radius_px: f32) -> Self {
        self.appearance.corner_radius_px = corner_radius_px;
        self
    }

    #[must_use]
    pub fn corner_radius(self) -> Radius {
        Radius::from(self.appearance.corner_radius_px)
    }

    #[must_use]
    pub fn drawn_corners(self, outlined_sides: OutlinedSides) -> Radius {
        outlined_sides.square_unoutlined_corners(self.corner_radius())
    }

    #[must_use]
    pub fn container_style(self, theme: &Theme) -> container::Style {
        container::Style::default()
            .background(self.fill_color(theme))
            .border(Border {
                radius: self.corner_radius(),
                ..self.outline_border(theme)
            })
            .shadow(self.drop_shadow())
            .color(text_color(theme))
    }

    #[must_use]
    pub fn fill_color(self, theme: &Theme) -> Color {
        let background = match self.appearance.background {
            LayerBackground::Base => background_color(theme),
            LayerBackground::Weak => background_weak_color(theme),
        };
        background.scale_alpha(self.appearance.opacity)
    }

    #[must_use]
    pub fn outline_border(self, theme: &Theme) -> Border {
        if !self.appearance.outline.drawn {
            return Border::default();
        }
        Border {
            color: background_strong_color(theme).scale_alpha(self.appearance.outline.opacity),
            width: self.outline_width,
            radius: Radius::default(),
        }
    }

    #[must_use]
    pub fn outline_width(self) -> f32 {
        if self.appearance.outline.drawn {
            self.outline_width
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn drop_shadow(self) -> Shadow {
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

    #[must_use]
    pub fn lifted_drop_shadow(self) -> Shadow {
        let centered_shadow = self.drop_shadow();
        Shadow {
            offset: Vector::new(0.0, centered_shadow.blur_radius * SHADOW_DROP_PER_SOFTNESS),
            ..centered_shadow
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
    pub fn all_when_floating(self, float_offset_px: f32) -> Self {
        if float_offset_px > 0.0 {
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
    frame: LayerFrameStyle,
) -> PanelFrame<'a, Message> {
    PanelFrame {
        content: content.into(),
        frame,
        outlined_sides: OutlinedSides::ALL,
        blurred: false,
    }
}

pub struct PanelFrame<'a, Message> {
    content: Element<'a, Message>,
    frame: LayerFrameStyle,
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
        self.frame.drawn_corners(self.outlined_sides)
    }

    fn blur_regions(&self, bounds: Rectangle) -> BlurRegions {
        if !self.blurred {
            return replacing_blur_regions(Vec::new());
        }
        replacing_blur_regions(rounded_blur_rows(bounds, self.drawn_radius()))
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
    use sottomano_config::screen_edges::Position;

    use super::{LayerFrameStyle, OutlinedSides, panel_frame};
    use crate::widgets::shell_layer_appearance::LayerAppearance;

    #[test]
    fn a_top_panel_blurs_its_screen_edge_corners_square() {
        let appearance = LayerAppearance {
            corner_radius_px: 8.0,
            ..LayerAppearance::default()
        };
        let frame = panel_frame::<()>(space(), LayerFrameStyle::new(&appearance))
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
