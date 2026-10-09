use chrono::Locale;
use iced::{
    Alignment, Element, Padding,
    mouse::ScrollDelta,
    time::Duration,
    widget::{Row, button, container, mouse_area},
};

use crate::{
    config::Config,
    data::severity::Severity,
    desktop_shell::{
        hover_tooltips::{HoverTarget, PressHints, TooltipMessage, hoverable},
        monitor_surfaces::Position,
    },
    widgets::{
        activity_line::activity_line,
        announced_level::LevelReading,
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

const EXPANSION_DURATION: Duration = Duration::from_millis(90);

#[derive(Debug, Clone, Copy)]
pub enum ExpansionMessage {
    Toggle,
    Expand,
    CursorEntered,
    CursorExited,
    Collapse,
    HandleScrolled(ScrollDelta),
}

#[derive(Default)]
pub struct GroupExpansion {
    expanded: bool,
    cursor_hovering: bool,
}

impl GroupExpansion {
    pub fn update(&mut self, msg: ExpansionMessage) -> bool {
        let was_expanded = self.is_expanded();
        match msg {
            ExpansionMessage::Toggle => self.expanded = !self.expanded,
            ExpansionMessage::Expand => self.expanded = true,
            ExpansionMessage::CursorEntered => self.cursor_hovering = true,
            ExpansionMessage::CursorExited => self.cursor_hovering = false,
            ExpansionMessage::Collapse => self.expanded = false,
            ExpansionMessage::HandleScrolled(_) => {}
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

pub struct GroupHandle<'a> {
    pub face: HandleFace<'a>,
    pub preview: Option<PreviewedValue<'a>>,
    pub forwards_scroll: bool,
}

pub enum HandleFace<'a> {
    Icon(Glyph),
    IconColoredBy {
        glyph: Glyph,
        reading: &'a dyn BarWidget,
    },
    FirstItem,
}

#[derive(Clone)]
pub enum PreviewedValue<'a> {
    Plain(&'a dyn BarWidget),
    Hoverable {
        widget: &'a dyn BarWidget,
        target: HoverTarget,
    },
}

impl<'a> PreviewedValue<'a> {
    fn widget(&self) -> &'a dyn BarWidget {
        match self {
            PreviewedValue::Plain(widget) | PreviewedValue::Hoverable { widget, .. } => *widget,
        }
    }

    fn hoverable_face<M: 'a>(
        &self,
        face: Element<'a, GroupEvent<M>>,
    ) -> Element<'a, GroupEvent<M>> {
        match self {
            PreviewedValue::Plain(_) => face,
            PreviewedValue::Hoverable { target, .. } => hoverable(
                face,
                target.clone(),
                PressHints {
                    left: Some("expand or collapse the group"),
                    ..PressHints::default()
                },
                GroupEvent::Tooltip,
            ),
        }
    }
}

pub struct GroupViewWithAlerts<'a, Outer> {
    pub element: Option<Element<'a, GroupEvent<Outer>>>,
    pub alerts: Vec<ReadingAlert<Outer>>,
    pub levels: Vec<LevelReading<Outer>>,
}

pub struct WidgetGroup<'a, M> {
    expansion: &'a GroupExpansion,
    on_expansion: fn(ExpansionMessage) -> M,
    face: HandleFace<'a>,
    items: Vec<GroupItem<'a, M>>,
    alerts: Vec<ReadingAlert<M>>,
    previewed_value: Option<PreviewedValue<'a>>,
    forward_handle_scroll: bool,
}

impl<'a, M: 'a> WidgetGroup<'a, M> {
    pub fn new<Item: Into<Option<GroupItem<'a, M>>> + Clone>(
        expansion: &'a GroupExpansion,
        on_expansion: fn(ExpansionMessage) -> M,
        handle: GroupHandle<'a>,
        items: &[Item],
    ) -> Self
    where
        M: Clone,
    {
        let items: Vec<GroupItem<'a, M>> = items.iter().cloned().filter_map(Into::into).collect();
        let alerts = items.iter().flat_map(|item| item.alerts.clone()).collect();
        Self {
            expansion,
            on_expansion,
            face: handle.face,
            items,
            alerts,
            previewed_value: handle.preview,
            forward_handle_scroll: handle.forwards_scroll,
        }
    }

    pub fn place_on_bar<Outer: 'a>(
        self,
        lift_message: fn(M) -> Outer,
        config: &'a Config,
        bar_position: Position,
    ) -> GroupViewWithAlerts<'a, Outer>
    where
        M: Clone,
    {
        let alerts = self.alerts(config.locale);
        let levels = self
            .items
            .iter()
            .filter_map(|item| item.level_reading((self.on_expansion)(ExpansionMessage::Expand)))
            .map(|level| level.map_messages(lift_message))
            .collect();
        let highest_alert_severity = alerts
            .iter()
            .map(|alert| alert.severity)
            .max()
            .unwrap_or_default();
        GroupViewWithAlerts {
            element: self
                .view(config, bar_position, highest_alert_severity)
                .map(|element| element.map(move |event| event.map(lift_message))),
            alerts: alerts
                .into_iter()
                .map(|alert| alert.map_messages(lift_message))
                .collect(),
            levels,
        }
    }

    fn alerts(&self, locale: Locale) -> Vec<ReadingAlert<M>>
    where
        M: Clone,
    {
        self.items
            .iter()
            .filter_map(|item| item.alert((self.on_expansion)(ExpansionMessage::Expand), locale))
            .chain(self.alerts.iter().cloned())
            .collect()
    }

    fn view(
        self,
        config: &'a Config,
        bar_position: Position,
        highest_alert_severity: Severity,
    ) -> Option<Element<'a, GroupEvent<M>>>
    where
        M: Clone,
    {
        let Self {
            expansion,
            on_expansion,
            face,
            mut items,
            alerts: _,
            previewed_value,
            forward_handle_scroll,
        } = self;
        if items.is_empty() {
            return None;
        }
        let face = match face {
            HandleFace::Icon(glyph) => GroupItem::element(move |_| {
                glyph.view(config.font.icon_size_px, config.palette.bar_foreground)
            }),
            HandleFace::IconColoredBy { glyph, reading } => GroupItem::element(move |_| {
                glyph.view(config.font.icon_size_px, reading_color(config, reading))
            }),
            HandleFace::FirstItem => items.remove(0),
        };
        let content = items;
        let draw_face = move || face.view(config, 1.0).unwrap_or_else(|| Row::new().into());
        let draw_content = move |reveal: f32| -> Element<'a, GroupEvent<M>> {
            Row::with_children(content.iter().filter_map(|item| item.view(config, reveal)))
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
        };
        let expansion_event = move |message| GroupEvent::Group(on_expansion(message));
        let toggle = expansion_event(ExpansionMessage::Toggle);
        let cursor_hovering = expansion.cursor_hovering;
        let expanded = expansion.expanded;
        let animated = expand_animation(expanded, EXPANSION_DURATION, move |reveal| {
            let handle: Element<'a, GroupEvent<M>> = button(
                Row::with_children([
                    draw_face(),
                    collapsing_previewed_value(
                        config,
                        previewed_value.as_ref().map(PreviewedValue::widget),
                        reveal,
                    ),
                ])
                .align_y(Alignment::Center),
            )
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(toggle.clone())
            .into();
            let handle = match &previewed_value {
                Some(preview) => preview.hoverable_face(handle),
                None => handle,
            };
            let handle = if forward_handle_scroll {
                mouse_area(handle)
                    .on_scroll(move |delta| {
                        expansion_event(ExpansionMessage::HandleScrolled(delta))
                    })
                    .into()
            } else {
                handle
            };
            let inner = Row::new()
                .push(handle)
                .extend(
                    (reveal > 0.0).then(|| revealed_content(config, reveal, draw_content(reveal))),
                )
                .align_y(Alignment::Center);
            PagerSlot::new(
                inner.into(),
                edge_indicator(
                    config,
                    highest_alert_severity,
                    cursor_hovering,
                    expanded,
                    reveal,
                ),
            )
            .view(config, bar_position)
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
    cursor_hovering: bool,
    expanded: bool,
    reveal: f32,
) -> Element<'a, Message> {
    let thickness = config.widget_sizes.indicator_thickness_px;
    let base = if cursor_hovering { 0.5 } else { 0.2 };
    let revealing_opacity = base + (1.0 - base) * reveal;
    match alert_severity {
        Severity::Low | Severity::Normal if expanded => {
            activity_line(config.palette.focus, revealing_opacity, thickness)
        }
        Severity::Low | Severity::Normal if cursor_hovering => {
            activity_line(config.palette.bar_foreground, base, thickness)
        }
        Severity::Low | Severity::Normal => {
            activity_line(config.palette.bar_foreground, 0.12, thickness)
        }
        Severity::Warning | Severity::Critical => activity_line(
            alert_severity.color(&config.palette),
            revealing_opacity,
            thickness,
        ),
    }
}
