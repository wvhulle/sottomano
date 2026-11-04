use iced::{
    Element, Event, Length, Rectangle, Shadow, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    widget::container,
};

use crate::widgets::typing_under_cards::CardBackgroundFade;

pub struct ToastCardBackdrop<'a, Message> {
    content: Element<'a, Message>,
    resting_style: container::Style,
    background_fade: CardBackgroundFade,
}

impl<'a, Message> ToastCardBackdrop<'a, Message> {
    pub fn new(
        content: impl Into<Element<'a, Message>>,
        resting_style: container::Style,
        background_fade: CardBackgroundFade,
    ) -> Self {
        Self {
            content: content.into(),
            resting_style,
            background_fade,
        }
    }
}

fn faded_backdrop_style(resting_style: &container::Style, fade_level: f32) -> container::Style {
    let remaining = 1.0 - fade_level;
    container::Style {
        background: resting_style
            .background
            .map(|background| background.scale_alpha(remaining)),
        shadow: Shadow {
            color: resting_style.shadow.color.scale_alpha(remaining),
            ..resting_style.shadow
        },
        ..*resting_style
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for ToastCardBackdrop<'_, Message> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
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
        let fade_level = self.background_fade.fade_level_over(layout.bounds());
        container::draw_background(
            renderer,
            &faded_backdrop_style(&self.resting_style, fade_level),
            layout.bounds(),
        );
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
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
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

impl<'a, Message: 'a> From<ToastCardBackdrop<'a, Message>> for Element<'a, Message> {
    fn from(backdrop: ToastCardBackdrop<'a, Message>) -> Self {
        Element::new(backdrop)
    }
}

#[cfg(test)]
mod tests {
    use iced::{Background, Color, Shadow, border, widget::container};

    use super::faded_backdrop_style;

    #[test]
    fn fading_scales_the_fill_and_shadow_but_keeps_the_outline() {
        let resting_style = container::Style::default()
            .background(Color::WHITE)
            .border(border::rounded(4.0).color(Color::WHITE).width(1.0))
            .shadow(Shadow {
                color: Color::BLACK,
                ..Shadow::default()
            });
        let faded = faded_backdrop_style(&resting_style, 0.75);
        let Some(Background::Color(fill)) = faded.background else {
            panic!("the fill stays a colour");
        };
        assert!((fill.a - 0.25).abs() <= 0.001);
        assert!((faded.shadow.color.a - 0.25).abs() <= 0.001);
        assert_eq!(faded.border, resting_style.border);
    }
}
