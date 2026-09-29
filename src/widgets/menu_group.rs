use iced::{
    Alignment, Element, Padding,
    time::Duration,
    widget::{Row, button, container, mouse_area},
};

use crate::{
    config::Config,
    data::severity::Severity,
    wayland_layers::surface::Position,
    widgets::{
        activity_line::activity_line,
        bar_widget::BarWidget,
        expand_animation::expand_animation,
        flat_button,
        pager_slot::PagerSlot,
        reveal::reveal_x,
        source_reading::{reading_color, reading_value},
    },
};

const EXPAND: Duration = Duration::from_millis(90);

#[derive(Debug, Clone, Copy)]
pub enum MenuMessage {
    Toggle,
    ExpandGroup,
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
            MenuMessage::ExpandGroup => self.expanded = true,
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

    pub fn slot<'a, Message, Face, Content, OnEvent>(
        &self,
        face: Face,
        content: Content,
        on_event: OnEvent,
    ) -> MenuSlot<'a, Face, Content, OnEvent>
    where
        Message: 'a + Clone,
        Face: Fn() -> Element<'a, Message> + 'a,
        Content: Fn(f32) -> Element<'a, Message> + 'a,
        OnEvent: Fn(MenuMessage) -> Message,
    {
        MenuSlot {
            expanded: self.expanded,
            hovered: self.hovered,
            face,
            content,
            on_event,
            previewed_value: None,
            edge_indicator_alert_severity: Severity::Normal,
        }
    }
}

pub struct MenuSlot<'a, Face, Content, OnEvent> {
    expanded: bool,
    hovered: bool,
    face: Face,
    content: Content,
    on_event: OnEvent,
    previewed_value: Option<&'a dyn BarWidget>,
    edge_indicator_alert_severity: Severity,
}

impl<'a, Face, Content, OnEvent> MenuSlot<'a, Face, Content, OnEvent> {
    #[must_use]
    pub fn previewed_value(mut self, widget: &'a dyn BarWidget) -> Self {
        self.previewed_value = Some(widget);
        self
    }

    #[must_use]
    pub fn edge_indicator_alert_severity(mut self, level: Severity) -> Self {
        self.edge_indicator_alert_severity = level;
        self
    }

    pub fn view<Message>(self, config: &'a Config, position: Position) -> Element<'a, Message>
    where
        Message: 'a + Clone,
        Face: Fn() -> Element<'a, Message> + 'a,
        Content: Fn(f32) -> Element<'a, Message> + 'a,
        OnEvent: Fn(MenuMessage) -> Message,
    {
        let Self {
            expanded,
            hovered,
            face,
            content,
            on_event,
            previewed_value,
            edge_indicator_alert_severity,
        } = self;
        let toggle = on_event(MenuMessage::Toggle);
        let animated = expand_animation(expanded, EXPAND, move |reveal| {
            let handle = button(
                Row::with_children([
                    face(),
                    collapsing_previewed_value(config, previewed_value, reveal),
                ])
                .align_y(Alignment::Center),
            )
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(toggle.clone());
            let inner = Row::new()
                .push(handle)
                .extend((reveal > 0.0).then(|| revealed_content(config, reveal, content(reveal))))
                .align_y(Alignment::Center);
            PagerSlot::new(
                inner.into(),
                edge_indicator(config, edge_indicator_alert_severity, hovered, reveal),
            )
            .view(config, position)
        });
        mouse_area(animated)
            .on_enter(on_event(MenuMessage::CursorEntered))
            .on_exit(on_event(MenuMessage::CursorExited))
            .into()
    }
}

fn collapsing_previewed_value<'a, Message: 'a>(
    config: &Config,
    previewed_value: Option<&dyn BarWidget>,
    reveal: f32,
) -> Element<'a, Message> {
    let hidden = 1.0 - reveal;
    let Some(value) = previewed_value
        .filter(|_| hidden > 0.0)
        .and_then(|widget| reading_value(config, widget, reading_color(config, widget)))
    else {
        return Row::new().into();
    };
    let gap = config.spacing.widget_spacing_px;
    reveal_x(container(value).padding(Padding::ZERO.left(gap)), hidden)
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
    edge_indicator_alert_severity: Severity,
    hovered: bool,
    reveal: f32,
) -> Element<'a, Message> {
    let base = if hovered { 0.5 } else { 0.2 };
    let color = match edge_indicator_alert_severity {
        Severity::Low | Severity::Normal => config.palette.focus,
        Severity::Warning | Severity::Critical => {
            edge_indicator_alert_severity.color(&config.palette)
        }
    };
    activity_line(
        color,
        base + (1.0 - base) * reveal,
        config.widgets.indicator_thickness_px,
    )
}
