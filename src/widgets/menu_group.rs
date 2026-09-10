use iced::{
    Alignment, Element, Padding,
    time::Duration,
    widget::{Row, container, mouse_area},
};

use crate::{
    config::Config,
    wayland_layers::surface::Position,
    widgets::{
        activity_line::activity_line, expand_animation::expand_animation, pager_slot::PagerSlot,
        reveal::reveal_x,
    },
};

const EXPAND: Duration = Duration::from_millis(90);

#[derive(Debug, Clone, Copy)]
pub enum MenuMessage {
    Toggle,
    CursorEntered,
    CursorExited,
    Close,
}

#[derive(Default)]
pub struct MenuGroup {
    expanded: bool,
    hovered: bool,
}

impl MenuGroup {
    pub fn update(&mut self, msg: MenuMessage) -> bool {
        let was_open = self.is_open();
        match msg {
            MenuMessage::Toggle => self.expanded = !self.expanded,
            MenuMessage::CursorEntered => self.hovered = true,
            MenuMessage::CursorExited => self.hovered = false,
            MenuMessage::Close => self.expanded = false,
        }
        was_open && !self.is_open()
    }

    pub fn close(&mut self) -> bool {
        self.update(MenuMessage::Close)
    }

    pub fn is_open(&self) -> bool {
        self.expanded
    }

    pub fn view<'a, Message: 'a + Clone, F>(
        &self,
        config: &'a Config,
        position: Position,
        handle: impl Fn() -> Element<'a, Message> + 'a,
        content: impl Fn(f32) -> Element<'a, Message> + 'a,
        to_message: F,
    ) -> Element<'a, Message>
    where
        F: Fn(MenuMessage) -> Message,
    {
        let hovered = self.hovered;
        let animated = expand_animation(self.expanded, EXPAND, move |reveal| {
            let inner =
                Row::with_children([handle(), revealed_content(config, reveal, content(reveal))])
                    .align_y(Alignment::Center);
            PagerSlot::new(inner.into(), edge_indicator(config, hovered, reveal))
                .view(config, position)
        });
        mouse_area(animated)
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
