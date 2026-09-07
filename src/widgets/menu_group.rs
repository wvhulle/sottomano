use iced::{
    Alignment, Animation, Element, Padding,
    animation::Easing,
    time::{Duration, Instant},
    widget::{Row, container, mouse_area},
};

use crate::{
    config::Config,
    wayland_layers::surface::Position,
    widgets::{activity_line::activity_line, pager_slot::PagerSlot, reveal::reveal_x},
};

const EXPAND: Duration = Duration::from_millis(90);

#[derive(Debug, Clone, Copy)]
pub enum MenuMessage {
    Toggle,
    CursorEntered,
    CursorExited,
    Close,
}

pub struct MenuGroup {
    expand: Animation<bool>,
    hovered: bool,
}

impl Default for MenuGroup {
    fn default() -> Self {
        Self {
            expand: Animation::new(false)
                .easing(Easing::EaseOutCubic)
                .duration(EXPAND),
            hovered: false,
        }
    }
}

impl MenuGroup {
    pub fn update(&mut self, msg: MenuMessage) -> bool {
        let was_open = self.is_open();
        match msg {
            MenuMessage::Toggle => self.expand.go_mut(!self.is_open(), Instant::now()),
            MenuMessage::CursorEntered => self.hovered = true,
            MenuMessage::CursorExited => self.hovered = false,
            MenuMessage::Close => self.expand.go_mut(false, Instant::now()),
        }
        was_open && !self.is_open()
    }

    pub fn close(&mut self) -> bool {
        self.update(MenuMessage::Close)
    }

    pub fn is_open(&self) -> bool {
        self.expand.value()
    }

    pub fn is_animating(&self) -> bool {
        self.expand.is_animating(Instant::now())
    }

    pub fn reveal(&self) -> f32 {
        self.expand.interpolate(0.0, 1.0, Instant::now())
    }

    pub fn view<'a, Message: 'a + Clone, F>(
        &self,
        config: &Config,
        position: Position,
        handle: Element<'a, Message>,
        content: Element<'a, Message>,
        to_message: F,
    ) -> Element<'a, Message>
    where
        F: Fn(MenuMessage) -> Message,
    {
        let reveal = self.reveal();
        let inner = Row::with_children([handle, revealed_content(config, reveal, content)])
            .align_y(Alignment::Center);
        let slot = PagerSlot::new(inner.into(), edge_indicator(config, self.hovered, reveal))
            .view(config, position);
        mouse_area(slot)
            .on_enter(to_message(MenuMessage::CursorEntered))
            .on_exit(to_message(MenuMessage::CursorExited))
            .into()
    }
}

fn revealed_content<'a, Message: 'a>(
    config: &Config,
    reveal: f32,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let gap = config.spacing.widget_spacing_px;
    reveal_x(container(content).padding(Padding::ZERO.left(gap)), reveal)
}

fn edge_indicator<'a, Message: 'a>(
    config: &Config,
    hovered: bool,
    reveal: f32,
) -> Element<'a, Message> {
    let base = if hovered { 0.5 } else { 0.2 };
    activity_line(
        config.palette.focus,
        base + (1.0 - base) * reveal,
        config.widgets.focus_underline_thickness_px,
    )
}
