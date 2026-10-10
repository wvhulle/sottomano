use std::rc::Rc;

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

pub struct GroupToastRequests<M> {
    pub alerts: Vec<ReadingAlert<M>>,
    pub levels: Vec<LevelReading<M>>,
}

pub struct GroupContent<'a, M> {
    expansion: &'a GroupExpansion,
    on_expansion: Rc<dyn Fn(ExpansionMessage) -> M + 'a>,
    face: HandleFace<'a>,
    preview: Option<PreviewedValue<'a>>,
    forwards_scroll: bool,
    items: Vec<GroupItem<'a, M>>,
}

impl<'a, M: Clone + 'a> GroupContent<'a, M> {
    pub fn new(
        expansion: &'a GroupExpansion,
        on_expansion: fn(ExpansionMessage) -> M,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> Self {
        Self {
            expansion,
            on_expansion: Rc::new(on_expansion),
            face: HandleFace::FirstItem,
            preview: None,
            forwards_scroll: false,
            items: items.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn face(self, face: HandleFace<'a>) -> Self {
        Self { face, ..self }
    }

    #[must_use]
    pub fn preview(self, preview: Option<PreviewedValue<'a>>) -> Self {
        Self { preview, ..self }
    }

    #[must_use]
    pub fn forwards_scroll(self, forwards_scroll: bool) -> Self {
        Self {
            forwards_scroll,
            ..self
        }
    }

    pub fn map<Outer: Clone + 'a>(self, lift: fn(M) -> Outer) -> GroupContent<'a, Outer> {
        let on_expansion = self.on_expansion;
        GroupContent {
            expansion: self.expansion,
            on_expansion: Rc::new(move |message| lift(on_expansion(message))),
            face: self.face,
            preview: self.preview,
            forwards_scroll: self.forwards_scroll,
            items: self.items.into_iter().map(|item| item.map(lift)).collect(),
        }
    }

    fn expand_message(&self) -> M {
        (self.on_expansion)(ExpansionMessage::Expand)
    }

    fn alerts(&self, locale: Locale) -> impl Iterator<Item = ReadingAlert<M>> {
        self.items
            .iter()
            .flat_map(move |item| item.alerts_with_reading(self.expand_message(), locale))
    }

    pub fn toast_requests(&self, locale: Locale) -> GroupToastRequests<M> {
        GroupToastRequests {
            alerts: self.alerts(locale).collect(),
            levels: self
                .items
                .iter()
                .filter_map(|item| item.level_reading(self.expand_message()))
                .collect(),
        }
    }

    pub fn view<Outer: From<GroupEvent<M>> + 'a>(
        self,
        config: &'a Config,
        bar_position: Position,
    ) -> Option<Element<'a, Outer>> {
        if self.items.is_empty() {
            return None;
        }
        let highest_alert_severity = self
            .alerts(config.locale)
            .map(|alert| alert.severity)
            .max()
            .unwrap_or_default();
        let GroupContent {
            expansion,
            on_expansion,
            face,
            preview,
            forwards_scroll,
            mut items,
        } = self;
        let face = match face {
            HandleFace::Icon(glyph) => GroupItem::element(move |_| {
                glyph.view(config.font.icon_size_px, config.palette.bar_foreground)
            }),
            HandleFace::IconColoredBy { glyph, reading } => GroupItem::element(move |_| {
                glyph.view(config.font.icon_size_px, reading_color(config, reading))
            }),
            HandleFace::FirstItem => items.remove(0),
        };
        let draw_face = move || face.view(config, 1.0).unwrap_or_else(|| Row::new().into());
        let draw_content = move |reveal: f32| -> Element<'a, GroupEvent<M>> {
            Row::with_children(items.iter().filter_map(|item| item.view(config, reveal)))
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into()
        };
        let expansion_event = move |message| GroupEvent::Group(on_expansion(message));
        let toggle = expansion_event(ExpansionMessage::Toggle);
        let on_enter = expansion_event(ExpansionMessage::CursorEntered);
        let on_exit = expansion_event(ExpansionMessage::CursorExited);
        let edge_indicator = EdgeIndicator {
            highest_alert_severity,
            cursor_hovering: expansion.cursor_hovering,
            expanded: expansion.expanded,
        };
        let animated = expand_animation(expansion.expanded, EXPANSION_DURATION, move |reveal| {
            let handle: Element<'a, GroupEvent<M>> = button(
                Row::with_children([
                    draw_face(),
                    collapsing_previewed_value(
                        config,
                        preview.as_ref().map(PreviewedValue::widget),
                        reveal,
                    ),
                ])
                .align_y(Alignment::Center),
            )
            .padding(0)
            .style(flat_button(config.palette.bar_foreground))
            .on_press(toggle.clone())
            .into();
            let handle = match &preview {
                Some(preview) => preview.hoverable_face(handle),
                None => handle,
            };
            let handle = if forwards_scroll {
                let expansion_event = expansion_event.clone();
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
            PagerSlot::new(inner.into(), edge_indicator.view(config, reveal))
                .view(config, bar_position)
        });
        Some(
            Element::from(mouse_area(animated).on_enter(on_enter).on_exit(on_exit))
                .map(Outer::from),
        )
    }
}

#[derive(Clone, Copy)]
struct EdgeIndicator {
    highest_alert_severity: Severity,
    cursor_hovering: bool,
    expanded: bool,
}

impl EdgeIndicator {
    fn view<'a, M: 'a>(self, config: &Config, reveal: f32) -> Element<'a, M> {
        let thickness = config.widget_sizes.indicator_thickness_px;
        let base = if self.cursor_hovering { 0.5 } else { 0.2 };
        let revealing_opacity = base + (1.0 - base) * reveal;
        match self.highest_alert_severity {
            Severity::Low | Severity::Normal if self.expanded => {
                activity_line(config.palette.focus, revealing_opacity, thickness)
            }
            Severity::Low | Severity::Normal if self.cursor_hovering => {
                activity_line(config.palette.bar_foreground, base, thickness)
            }
            Severity::Low | Severity::Normal => {
                activity_line(config.palette.bar_foreground, 0.12, thickness)
            }
            severity @ (Severity::Warning | Severity::Critical) => activity_line(
                severity.color(&config.palette),
                revealing_opacity,
                thickness,
            ),
        }
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
