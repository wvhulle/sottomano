use std::rc::Rc;

use chrono::Locale;
use iced::{
    Alignment, Element, Padding, Subscription,
    mouse::ScrollDelta,
    widget::{Row, button, container, mouse_area},
};

use crate::{
    config::Config,
    data::severity::Severity,
    desktop_shell::{
        hover_tooltips::{GroupHandle, HoverTarget, PressHints, TooltipMessage, hoverable},
        monitor_surfaces::Position,
    },
    widgets::{
        activity_line::activity_line,
        announced_level::LevelReading,
        bar_widget::{BarWidget, Glyph},
        expand_animation::expand_animation,
        flat_button,
        group_item::GroupItem,
        ongoing_toast::OngoingToast,
        pager_slot::PagerSlot,
        reveal::reveal_x,
        source_reading::{icon_value_gap, reading_color, reading_value},
    },
};

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

    pub fn while_expanded<M>(&self, subscription: Subscription<M>) -> Subscription<M> {
        if self.expanded {
            subscription
        } else {
            Subscription::none()
        }
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
    Icon {
        glyph: Glyph,
        handle: GroupHandle,
    },
    IconColoredBy {
        glyph: Glyph,
        reading: &'a dyn BarWidget,
        handle: GroupHandle,
    },
    FirstItem,
}

impl<'a> HandleFace<'a> {
    fn hover_target(&self) -> Option<HoverTarget> {
        match self {
            HandleFace::Icon { handle, .. } | HandleFace::IconColoredBy { handle, .. } => {
                Some(HoverTarget::GroupHandle(*handle))
            }
            HandleFace::FirstItem => None,
        }
    }

    fn into_face_item<M: Clone + 'a>(
        self,
        config: &'a Config,
        items: &mut Vec<GroupItem<'a, M>>,
        handle_hints: PressHints,
    ) -> GroupItem<'a, M> {
        match self {
            HandleFace::Icon { glyph, .. } => GroupItem::element(move |_| {
                glyph.view(
                    config.theme.font.icon_size_px,
                    config.theme.palette.bar_foreground,
                )
            }),
            HandleFace::IconColoredBy { glyph, reading, .. } => GroupItem::element(move |_| {
                glyph.view(
                    config.theme.font.icon_size_px,
                    reading_color(config, reading),
                )
            }),
            HandleFace::FirstItem => items.remove(0).acting_as_handle(handle_hints),
        }
    }
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

    fn hover_target(&self) -> Option<HoverTarget> {
        match self {
            PreviewedValue::Plain(_) => None,
            PreviewedValue::Hoverable { target, .. } => Some(target.clone()),
        }
    }
}

pub struct GroupToastRequests<M> {
    pub ongoing_toasts: Vec<OngoingToast<M>>,
    pub levels: Vec<LevelReading<M>>,
}

impl<M> Default for GroupToastRequests<M> {
    fn default() -> Self {
        Self {
            ongoing_toasts: Vec::new(),
            levels: Vec::new(),
        }
    }
}

impl<M> FromIterator<GroupToastRequests<M>> for GroupToastRequests<M> {
    fn from_iter<I: IntoIterator<Item = GroupToastRequests<M>>>(groups: I) -> Self {
        groups.into_iter().fold(
            GroupToastRequests {
                ongoing_toasts: Vec::new(),
                levels: Vec::new(),
            },
            |mut requests, group| {
                requests.ongoing_toasts.extend(group.ongoing_toasts);
                requests.levels.extend(group.levels);
                requests
            },
        )
    }
}

enum Handle<'a, M> {
    Expandable {
        expansion: &'a GroupExpansion,
        on_expansion: Rc<dyn Fn(ExpansionMessage) -> M + 'a>,
        face: HandleFace<'a>,
        preview: Option<PreviewedValue<'a>>,
        scroll_does: Option<&'static str>,
    },
    Fixed,
}

impl<'a, M: 'a> Handle<'a, M> {
    fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> Handle<'a, Outer> {
        match self {
            Handle::Expandable {
                expansion,
                on_expansion,
                face,
                preview,
                scroll_does,
            } => Handle::Expandable {
                expansion,
                on_expansion: Rc::new(move |message| lift(on_expansion(message))),
                face,
                preview,
                scroll_does,
            },
            Handle::Fixed => Handle::Fixed,
        }
    }

    fn expand_message(&self) -> Option<M> {
        match self {
            Handle::Expandable { on_expansion, .. } => Some(on_expansion(ExpansionMessage::Expand)),
            Handle::Fixed => None,
        }
    }
}

pub struct GroupContent<'a, M> {
    handle: Handle<'a, M>,
    items: Vec<GroupItem<'a, M>>,
}

impl<'a, M: Clone + 'a> GroupContent<'a, M> {
    pub fn new(
        expansion: &'a GroupExpansion,
        on_expansion: fn(ExpansionMessage) -> M,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> Self {
        Self {
            handle: Handle::Expandable {
                expansion,
                on_expansion: Rc::new(on_expansion),
                face: HandleFace::FirstItem,
                preview: None,
                scroll_does: None,
            },
            items: items.into_iter().collect(),
        }
    }

    pub fn single(item: Option<GroupItem<'a, M>>) -> Self {
        Self {
            handle: Handle::Fixed,
            items: item.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn face(mut self, face: HandleFace<'a>) -> Self {
        if let Handle::Expandable { face: current, .. } = &mut self.handle {
            *current = face;
        }
        self
    }

    #[must_use]
    pub fn preview(mut self, preview: Option<PreviewedValue<'a>>) -> Self {
        if let Handle::Expandable {
            preview: current, ..
        } = &mut self.handle
        {
            *current = preview;
        }
        self
    }

    #[must_use]
    pub fn forwards_scroll(mut self, scroll_does: Option<&'static str>) -> Self {
        if let Handle::Expandable {
            scroll_does: current,
            ..
        } = &mut self.handle
        {
            *current = scroll_does;
        }
        self
    }

    pub fn map<Outer: Clone + 'a>(self, lift: fn(M) -> Outer) -> GroupContent<'a, Outer> {
        GroupContent {
            handle: self.handle.map(lift),
            items: self.items.into_iter().map(|item| item.map(lift)).collect(),
        }
    }

    fn ongoing_toasts(&self, locale: Locale) -> impl Iterator<Item = OngoingToast<M>> {
        self.handle
            .expand_message()
            .into_iter()
            .flat_map(|expand| {
                self.items
                    .iter()
                    .flat_map(move |item| item.ongoing_toasts_with_reading(expand.clone(), locale))
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    pub fn toast_requests(&self, locale: Locale) -> GroupToastRequests<M> {
        let Some(expand) = self.handle.expand_message() else {
            return GroupToastRequests::default();
        };
        GroupToastRequests {
            ongoing_toasts: self.ongoing_toasts(locale).collect(),
            levels: self
                .items
                .iter()
                .filter_map(|item| item.level_reading(expand.clone()))
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
            .ongoing_toasts(config.calendar.locale)
            .map(|toast| toast.severity)
            .max()
            .unwrap_or_default();
        let GroupContent { handle, mut items } = self;
        let Handle::Expandable {
            expansion,
            on_expansion,
            face,
            preview,
            scroll_does,
        } = handle
        else {
            return Some(fixed_view(
                &items,
                highest_alert_severity,
                config,
                bar_position,
            ));
        };
        let handle_hints = PressHints::group_handle(expansion.expanded, scroll_does);
        let handle_target = preview
            .as_ref()
            .and_then(PreviewedValue::hover_target)
            .or(face.hover_target());
        let face = face.into_face_item(config, &mut items, handle_hints);
        let draw_face = move || face.view(config, 1.0);
        let draw_content = move |reveal: f32| -> Element<'a, GroupEvent<M>> {
            Row::with_children(items.iter().map(|item| item.view(config, reveal)))
                .spacing(config.layout.panel.widget_spacing)
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
        let expansion_duration = config.panel_bar.group_expansion;
        let animated = expand_animation(expansion.expanded, expansion_duration, move |reveal| {
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
            .style(flat_button(config.theme.palette.bar_foreground))
            .on_press(toggle.clone())
            .into();
            let handle = match &handle_target {
                Some(target) => {
                    hoverable(handle, target.clone(), handle_hints, GroupEvent::Tooltip)
                }
                None => handle,
            };
            let handle = if scroll_does.is_some() {
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

fn fixed_view<'a, M: Clone + 'a, Outer: From<GroupEvent<M>> + 'a>(
    items: &[GroupItem<'a, M>],
    highest_alert_severity: Severity,
    config: &'a Config,
    bar_position: Position,
) -> Element<'a, Outer> {
    let row = Row::with_children(items.iter().map(|item| item.view(config, 1.0)))
        .spacing(config.layout.panel.widget_spacing)
        .align_y(Alignment::Center);
    let indicator = EdgeIndicator {
        highest_alert_severity,
        cursor_hovering: false,
        expanded: false,
    }
    .view(config, 0.0);
    PagerSlot::new(row.into(), indicator)
        .view(config, bar_position)
        .map(Outer::from)
}

#[derive(Clone, Copy)]
struct EdgeIndicator {
    highest_alert_severity: Severity,
    cursor_hovering: bool,
    expanded: bool,
}

impl EdgeIndicator {
    fn view<'a, M: 'a>(self, config: &Config, reveal: f32) -> Element<'a, M> {
        let thickness = config.panel_bar.indicator_thickness;
        let base = if self.cursor_hovering { 0.5 } else { 0.2 };
        let revealing_opacity = base + (1.0 - base) * reveal;
        match self.highest_alert_severity {
            Severity::Low | Severity::Normal if self.expanded => {
                activity_line(config.theme.palette.focus, revealing_opacity, thickness)
            }
            Severity::Low | Severity::Normal if self.cursor_hovering => {
                activity_line(config.theme.palette.bar_foreground, base, thickness)
            }
            Severity::Low | Severity::Normal => {
                activity_line(config.theme.palette.bar_foreground, 0.12, thickness)
            }
            severity @ (Severity::Warning | Severity::Critical) => activity_line(
                severity.color(&config.theme.palette),
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
    let gap = config.layout.panel.widget_spacing;
    reveal_x(container(content).padding(Padding::ZERO.left(gap)), reveal)
}
