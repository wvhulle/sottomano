use std::rc::Rc;

use iced::{
    Alignment, Element, Padding,
    time::Duration,
    widget::{Row, button, container, mouse_area},
};

use crate::{
    config::Config,
    data::severity::Severity,
    wayland_layers::{
        surface::Position,
        tooltip::{HoverTarget, TooltipMessage},
    },
    widgets::{
        activity_line::activity_line,
        bar_widget::{BarWidget, Glyph},
        expand_animation::expand_animation,
        flat_button,
        group_item::GroupItem,
        pager_slot::PagerSlot,
        reading_alert::ReadingAlert,
        reveal::reveal_x,
        source_reading::{icon_value_gap, reading_color, reading_value},
    },
};

const EXPAND: Duration = Duration::from_millis(90);

#[derive(Debug, Clone, Copy)]
pub enum ExpansionMessage {
    Toggle,
    Expand,
    CursorEntered,
    CursorExited,
    Collapse,
}

#[derive(Default)]
pub struct GroupExpansion {
    expanded: bool,
    hovered: bool,
}

impl GroupExpansion {
    pub fn update(&mut self, msg: ExpansionMessage) -> bool {
        let was_expanded = self.is_expanded();
        match msg {
            ExpansionMessage::Toggle => self.expanded = !self.expanded,
            ExpansionMessage::Expand => self.expanded = true,
            ExpansionMessage::CursorEntered => self.hovered = true,
            ExpansionMessage::CursorExited => self.hovered = false,
            ExpansionMessage::Collapse => self.expanded = false,
        }
        was_expanded && !self.is_expanded()
    }

    pub fn collapse(&mut self) -> bool {
        self.update(ExpansionMessage::Collapse)
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }
}

#[derive(Debug, Clone)]
pub enum GroupEvent<M> {
    Group(M),
    Tooltip(TooltipMessage),
}

impl<M> GroupEvent<M> {
    pub fn map<Lifted>(self, lift: impl FnOnce(M) -> Lifted) -> GroupEvent<Lifted> {
        match self {
            GroupEvent::Group(message) => GroupEvent::Group(lift(message)),
            GroupEvent::Tooltip(tooltip) => GroupEvent::Tooltip(tooltip),
        }
    }
}

#[derive(Clone, Copy)]
enum GroupIcon<'a> {
    Plain(Glyph),
    ColoredByWidget {
        glyph: Glyph,
        widget: &'a dyn BarWidget,
    },
}

impl GroupIcon<'_> {
    fn view<'a, M: 'a>(self, config: &Config) -> Element<'a, M> {
        match self {
            GroupIcon::Plain(glyph) => {
                glyph.view(config.font.icon_size_px, config.palette.bar_foreground)
            }
            GroupIcon::ColoredByWidget { glyph, widget } => {
                glyph.view(config.font.icon_size_px, reading_color(config, widget))
            }
        }
    }
}

pub struct WidgetGroup<'a, M> {
    expansion: &'a GroupExpansion,
    on_expansion: Rc<dyn Fn(ExpansionMessage) -> M + 'a>,
    icon: Option<GroupIcon<'a>>,
    items: Vec<GroupItem<'a, M>>,
    previewed_value: Option<&'a dyn BarWidget>,
}

impl<'a, M: 'a> WidgetGroup<'a, M> {
    pub fn new(
        expansion: &'a GroupExpansion,
        on_expansion: impl Fn(ExpansionMessage) -> M + 'a,
    ) -> Self {
        Self {
            expansion,
            on_expansion: Rc::new(on_expansion),
            icon: None,
            items: Vec::new(),
            previewed_value: None,
        }
    }

    #[must_use]
    pub fn icon(mut self, icon: impl Into<Glyph>) -> Self {
        self.icon = Some(GroupIcon::Plain(icon.into()));
        self
    }

    #[must_use]
    pub fn icon_colored_by(mut self, icon: impl Into<Glyph>, widget: &'a dyn BarWidget) -> Self {
        self.icon = Some(GroupIcon::ColoredByWidget {
            glyph: icon.into(),
            widget,
        });
        self
    }

    #[must_use]
    pub fn item(mut self, item: impl Into<Option<GroupItem<'a, M>>>) -> Self {
        self.items.extend(item.into());
        self
    }

    #[must_use]
    pub fn reading(self, widget: Option<&'a dyn BarWidget>, target: HoverTarget) -> Self {
        self.item(widget.map(|widget| GroupItem::reading(widget, target)))
    }

    #[must_use]
    pub fn items(mut self, items: impl IntoIterator<Item = GroupItem<'a, M>>) -> Self {
        self.items.extend(items);
        self
    }

    #[must_use]
    pub fn previewed_value(mut self, widget: Option<&'a dyn BarWidget>) -> Self {
        self.previewed_value = widget;
        self
    }

    pub fn map_messages<Lifted: 'a>(
        self,
        lift: impl Fn(M) -> Lifted + 'a,
    ) -> WidgetGroup<'a, Lifted> {
        let lift: Rc<dyn Fn(M) -> Lifted + 'a> = Rc::new(lift);
        let on_expansion = self.on_expansion;
        WidgetGroup {
            expansion: self.expansion,
            on_expansion: Rc::new({
                let lift = Rc::clone(&lift);
                move |message| lift(on_expansion(message))
            }),
            icon: self.icon,
            items: self
                .items
                .into_iter()
                .map(|item| item.map_messages(&lift))
                .collect(),
            previewed_value: self.previewed_value,
        }
    }

    pub fn alerts(&self) -> Vec<ReadingAlert<M>>
    where
        M: Clone,
    {
        self.items
            .iter()
            .filter_map(|item| item.alert((self.on_expansion)(ExpansionMessage::Expand)))
            .collect()
    }

    fn highest_alert_severity(&self) -> Severity
    where
        M: Clone,
    {
        self.alerts()
            .iter()
            .map(|alert| alert.severity)
            .max()
            .unwrap_or_default()
    }

    pub fn view(self, config: &'a Config, position: Position) -> Option<Element<'a, GroupEvent<M>>>
    where
        M: Clone,
    {
        let edge_severity = self.highest_alert_severity();
        let Self {
            expansion,
            on_expansion,
            icon,
            items,
            previewed_value,
        } = self;
        let mut items = items.into_iter().peekable();
        items.peek()?;
        let face = match icon {
            Some(icon) => GroupItem::element(move |_| icon.view(config)),
            None => items.next()?,
        };
        let content: Vec<GroupItem<'a, M>> = items.collect();
        let draw_face = move || {
            face.clone()
                .view(config, 1.0)
                .unwrap_or_else(|| Row::new().into())
        };
        let draw_content = move |reveal: f32| -> Element<'a, GroupEvent<M>> {
            Row::with_children(
                content
                    .iter()
                    .cloned()
                    .filter_map(|item| item.view(config, reveal)),
            )
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center)
            .into()
        };
        let expansion_event = move |message| GroupEvent::Group(on_expansion(message));
        let toggle = expansion_event(ExpansionMessage::Toggle);
        let hovered = expansion.hovered;
        let animated = expand_animation(expansion.expanded, EXPAND, move |reveal| {
            let handle = button(
                Row::with_children([
                    draw_face(),
                    collapsing_previewed_value(config, previewed_value, reveal),
                ])
                .align_y(Alignment::Center),
            )
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(toggle.clone());
            let inner = Row::new()
                .push(handle)
                .extend(
                    (reveal > 0.0).then(|| revealed_content(config, reveal, draw_content(reveal))),
                )
                .align_y(Alignment::Center);
            PagerSlot::new(
                inner.into(),
                edge_indicator(config, edge_severity, hovered, reveal),
            )
            .view(config, position)
        });
        Some(
            mouse_area(animated)
                .on_enter(expansion_event(ExpansionMessage::CursorEntered))
                .on_exit(expansion_event(ExpansionMessage::CursorExited))
                .into(),
        )
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
    reveal_x(
        container(value).padding(Padding::ZERO.left(icon_value_gap(config))),
        hidden,
    )
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
    alert_severity: Severity,
    hovered: bool,
    reveal: f32,
) -> Element<'a, Message> {
    let base = if hovered { 0.5 } else { 0.2 };
    let color = match alert_severity {
        Severity::Low | Severity::Normal => config.palette.focus,
        Severity::Warning | Severity::Critical => alert_severity.color(&config.palette),
    };
    activity_line(
        color,
        base + (1.0 - base) * reveal,
        config.widgets.indicator_thickness_px,
    )
}
