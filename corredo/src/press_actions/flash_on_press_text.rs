use core::slice;
use std::time::{Duration, Instant};

use iced::{
    Border, Color, Element, Event, Length, Rectangle, Size, Theme,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::{Cursor, Interaction},
    widget::container,
    window,
};

use crate::{
    pointer_hover::{PointerHover, hover_over_bounds},
    theming::{
        AccentShades, primary_shades,
        style_metrics::{OUTLINE_WIDTH_PX, PRESSABLE_OUTLINE_PADDING, SMALL_CORNER_RADIUS_PX},
    },
};

const DEFAULT_HOLD: Duration = Duration::from_millis(700);

pub fn flash_on_press_text<'view, PressPayload: Clone + 'view, Message: 'view>(
    content: impl Into<Element<'view, Message>>,
    payload: PressPayload,
    on_press: impl Fn(PressPayload) -> Message + 'view,
) -> FlashOnPressText<'view, PressPayload, Message> {
    FlashOnPressText {
        content: container(content).padding(PRESSABLE_OUTLINE_PADDING).into(),
        payload,
        on_press: Box::new(on_press),
        accent: primary_shades,
        hold: DEFAULT_HOLD,
        interaction: Interaction::Pointer,
    }
}

impl<PressPayload, Message> FlashOnPressText<'_, PressPayload, Message> {
    #[must_use]
    pub fn accent(self, accent: fn(&Theme) -> AccentShades) -> Self {
        Self { accent, ..self }
    }

    #[must_use]
    pub fn hold(self, hold: Duration) -> Self {
        Self { hold, ..self }
    }

    #[must_use]
    pub fn interaction(self, interaction: Interaction) -> Self {
        Self {
            interaction,
            ..self
        }
    }
}

impl<'view, PressPayload: Clone + 'view, Message: 'view>
    From<FlashOnPressText<'view, PressPayload, Message>> for Element<'view, Message>
{
    fn from(pressable: FlashOnPressText<'view, PressPayload, Message>) -> Self {
        Element::new(pressable)
    }
}

pub struct FlashOnPressText<'view, PressPayload, Message> {
    content: Element<'view, Message>,
    payload: PressPayload,
    on_press: Box<dyn Fn(PressPayload) -> Message + 'view>,
    accent: fn(&Theme) -> AccentShades,
    hold: Duration,
    interaction: Interaction,
}

#[derive(Default)]
enum Press {
    #[default]
    Idle,
    Pending,
    Held {
        since: Instant,
        latest_frame: Instant,
    },
}

#[derive(Default)]
struct CellState {
    pointer_hover: PointerHover,
    press: Press,
}

impl CellState {
    fn hold_phase(&self, hold: Duration) -> HoldPhase {
        let Press::Held {
            since,
            latest_frame,
        } = &self.press
        else {
            return HoldPhase::Released;
        };
        let held_for = latest_frame
            .checked_duration_since(*since)
            .expect("a press is held from an earlier redraw frame");
        if held_for < hold {
            HoldPhase::Holding
        } else {
            HoldPhase::Released
        }
    }
}

impl<PressPayload: Clone, Message> Widget<Message, iced::Theme, iced::Renderer>
    for FlashOnPressText<'_, PressPayload, Message>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<CellState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(CellState::default())
    }

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
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(child.size(), vec![child])
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
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let pointer_hover = hover_over_bounds(cursor, layout.bounds());
        let state = tree.state.downcast_mut::<CellState>();
        if state.pointer_hover != pointer_hover {
            state.pointer_hover = pointer_hover;
            shell.request_redraw();
        }
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            match &mut state.press {
                Press::Idle => {}
                Press::Pending => {
                    state.press = Press::Held {
                        since: *now,
                        latest_frame: *now,
                    };
                    shell.request_redraw_at(*now + self.hold);
                }
                Press::Held { latest_frame, .. } => *latest_frame = *now,
            }
        }
        if shell.is_event_captured() || pointer_hover == PointerHover::Outside {
            return;
        }
        if !matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            return;
        }
        state.press = Press::Pending;
        shell.publish((self.on_press)(self.payload.clone()));
        shell.capture_event();
        shell.request_redraw();
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<CellState>();
        let hold_phase = state.hold_phase(self.hold);
        if hold_phase == HoldPhase::Holding || state.pointer_hover == PointerHover::Over {
            let accent = (self.accent)(theme);
            let (outline_color, fill_color) = match hold_phase {
                HoldPhase::Holding => (accent.base, accent.weak),
                HoldPhase::Released => (accent.weak, Color::TRANSPARENT),
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        color: outline_color,
                        width: OUTLINE_WIDTH_PX,
                        radius: Radius::new(SMALL_CORNER_RADIUS_PX),
                    },
                    ..renderer::Quad::default()
                },
                fill_color,
            );
        }
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
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
    ) -> Interaction {
        if cursor.is_over(layout.bounds()) {
            return self.interaction;
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<overlay::Element<'tree, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoldPhase {
    Holding,
    Released,
}
