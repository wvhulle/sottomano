use std::{collections::BTreeMap, rc::Rc};

use chrono::Locale;
use iced::{
    Alignment, Element, Length, Padding, Subscription,
    mouse::ScrollDelta,
    widget::{Row, Space, button, container, mouse_area},
};
use sottomano_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    hover_tooltips::{
        GroupHandle, HoverAnnouncement, HoverTarget, InspectedRow, PanelPin, PressHints,
        TooltipMessage, announcing_hoverable,
    },
    widgets::{
        activity_line::activity_line,
        aligned_quantity::{AlignedQuantity, ItemAlignment},
        announced_level::LevelReading,
        bar_widget::{BarWidget, Glyph},
        configured_theme::{primary_color, secondary_text_color, text_color},
        expand_animation::expand_animation,
        flat_button,
        group_item::{GroupItem, MenuParts},
        ongoing_toast::OngoingToast,
        pager_slot::PagerSlot,
        reveal::reveal_x,
        source_reading::{icon_value_gap, reading_value},
        widget_state::{StateTint, WidgetState},
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

    #[must_use]
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
        handle_announcement: HoverAnnouncement,
        clock: Clock,
    ) -> GroupItem<'a, M> {
        match self {
            HandleFace::Icon { glyph, .. } => {
                GroupItem::element(move |_| glyph.view(config.theme.font.icon_size_px, text_color))
            }
            HandleFace::IconColoredBy { glyph, reading, .. } => GroupItem::element(move |_| {
                glyph.view(config.theme.font.icon_size_px, {
                    let state = reading.state(clock);
                    move |theme: &iced::Theme| state.color(theme)
                })
            }),
            HandleFace::FirstItem => items.remove(0).acting_as_handle(handle_announcement),
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
                scroll_does,
            } => Handle::Expandable {
                expansion,
                on_expansion: Rc::new(move |message| lift(on_expansion(message))),
                face,
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

pub(super) struct MenuMember<'a, M> {
    pub group_title: &'static str,
    pub face: MenuParts<'a, M>,
    pub aligned: BTreeMap<AlignedQuantity, Element<'a, GroupEvent<M>>>,
    pub cascade_items: Vec<CascadeItem<'a, M>>,
}

pub(super) struct CascadeItem<'a, M> {
    pub name: &'static str,
    pub inspected_row: Option<InspectedRow>,
    pub press: Option<M>,
    pub parts: MenuParts<'a, M>,
}

pub trait WidgetGroup {
    const TITLE: &'static str;

    fn expandable_group<'a, M: Clone + 'a>(
        expansion: &'a GroupExpansion,
        on_expansion: impl Fn(ExpansionMessage) -> M + 'a,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> GroupContent<'a, M> {
        GroupContent::new(Self::TITLE, expansion, on_expansion, items)
    }

    fn fixed_group<'a, M: Clone + 'a>(item: Option<GroupItem<'a, M>>) -> GroupContent<'a, M> {
        GroupContent::single(Self::TITLE, item)
    }
}

pub struct GroupContent<'a, M> {
    group_title: &'static str,
    handle: Handle<'a, M>,
    items: Vec<GroupItem<'a, M>>,
    panel_pin: PanelPin,
}

impl<'a, M: Clone + 'a> GroupContent<'a, M> {
    fn new(
        group_title: &'static str,
        expansion: &'a GroupExpansion,
        on_expansion: impl Fn(ExpansionMessage) -> M + 'a,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> Self {
        Self {
            group_title,
            handle: Handle::Expandable {
                expansion,
                on_expansion: Rc::new(on_expansion),
                face: HandleFace::FirstItem,
                scroll_does: None,
            },
            items: items.into_iter().collect(),
            panel_pin: PanelPin::Unpinned,
        }
    }

    fn single(group_title: &'static str, item: Option<GroupItem<'a, M>>) -> Self {
        Self {
            group_title,
            handle: Handle::Fixed,
            items: item.into_iter().collect(),
            panel_pin: PanelPin::Unpinned,
        }
    }

    #[must_use]
    pub fn pinned_from(self, group_menu: GroupHandle) -> Self {
        Self {
            panel_pin: PanelPin::PinnedFrom(group_menu),
            ..self
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

    #[must_use]
    pub fn only_aligned_items(self) -> Self {
        self.retaining_items(|alignment| alignment != ItemAlignment::Unaligned)
    }

    #[must_use]
    pub fn only_unaligned_items(self) -> Self {
        self.retaining_items(|alignment| alignment == ItemAlignment::Unaligned)
    }

    fn retaining_items(mut self, keeps_alignment: impl Fn(ItemAlignment) -> bool) -> Self {
        self.items.retain(|item| keeps_alignment(item.alignment()));
        self
    }

    pub fn map<Outer: Clone + 'a>(self, lift: fn(M) -> Outer) -> GroupContent<'a, Outer> {
        GroupContent {
            group_title: self.group_title,
            handle: self.handle.map(lift),
            items: self.items.into_iter().map(|item| item.map(lift)).collect(),
            panel_pin: self.panel_pin,
        }
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests<M> {
        match self.handle.expand_message() {
            Some(expand) => self.toast_requests_revealed_by(&expand, locale, clock),
            None => GroupToastRequests::default(),
        }
    }

    fn toast_requests_revealed_by(
        &self,
        reveal_message: &M,
        locale: Locale,
        clock: Clock,
    ) -> GroupToastRequests<M> {
        GroupToastRequests {
            ongoing_toasts: self
                .items
                .iter()
                .flat_map(|item| {
                    item.ongoing_toasts_with_reading(reveal_message.clone(), locale, clock)
                })
                .collect(),
            levels: self
                .items
                .iter()
                .filter_map(|item| item.level_reading(reveal_message.clone(), locale, clock))
                .collect(),
        }
    }

    pub(super) fn highest_alert_severity(&self, clock: Clock) -> Severity {
        self.handle
            .expand_message()
            .and_then(|_| {
                self.items
                    .iter()
                    .filter_map(|item| item.ongoing_alert_severity(clock))
                    .max()
            })
            .unwrap_or_default()
    }

    pub(super) fn group_title(&self) -> &'static str {
        self.group_title
    }

    pub(super) fn menu_member(self, config: &'a Config, clock: Clock) -> Option<MenuMember<'a, M>> {
        if self.items.is_empty() {
            return None;
        }
        let GroupContent {
            group_title,
            handle,
            mut items,
            ..
        } = self;
        let face = match handle {
            Handle::Expandable { face, .. } => face,
            Handle::Fixed => HandleFace::FirstItem,
        };
        let face_item = match face {
            HandleFace::FirstItem => None,
            icon_face => Some(icon_face.into_face_item(
                config,
                &mut items,
                HoverAnnouncement::default(),
                clock,
            )),
        };
        let face = match &face_item {
            None => items[0].menu_parts(config, clock),
            Some(icon_item) => MenuParts {
                icon: icon_item.glance_view(config, clock),
                value: collapsing_previewed_value(config, &items, 0.0, clock),
            },
        };
        let (aligned, cascade_items) = items.iter().fold(
            (BTreeMap::new(), Vec::new()),
            |(mut aligned, mut cascade_items), item| {
                match item.alignment() {
                    ItemAlignment::Aligned(quantity) => {
                        aligned.insert(quantity, item.table_cell_view(config, clock));
                    }
                    ItemAlignment::Unaligned => cascade_items.push(CascadeItem {
                        name: item.menu_name().unwrap_or_default(),
                        inspected_row: item.inspected_row(),
                        press: item.press_message(),
                        parts: item.menu_parts(config, clock),
                    }),
                }
                (aligned, cascade_items)
            },
        );
        Some(MenuMember {
            group_title,
            face,
            aligned,
            cascade_items,
        })
    }

    pub fn view<Outer: From<GroupEvent<M>> + 'a>(
        self,
        config: &'a Config,
        bar_position: Position,
        clock: Clock,
    ) -> Option<Element<'a, Outer>> {
        if self.items.is_empty() {
            return None;
        }
        let highest_alert_severity = self.highest_alert_severity(clock);
        let GroupContent {
            group_title,
            handle,
            mut items,
            panel_pin,
        } = self;
        let Handle::Expandable {
            expansion,
            on_expansion,
            face,
            scroll_does,
        } = handle
        else {
            return Some(fixed_view(
                &items,
                highest_alert_severity,
                config,
                bar_position,
                clock,
            ));
        };
        let handle_announcement = HoverAnnouncement::from_group_handle(
            group_title,
            PressHints::group_handle(expansion.expanded, scroll_does),
        )
        .pinned_by(panel_pin);
        let face_target = face.hover_target();
        let face = face.into_face_item(config, &mut items, handle_announcement, clock);
        let handle_target = items
            .iter()
            .find_map(GroupItem::handle_preview)
            .map(|preview| preview.target.clone())
            .or(face_target);
        let draw_face = move || face.view(config, 1.0, clock);
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
                    collapsing_previewed_value(config, &items, reveal, clock),
                ])
                .align_y(Alignment::Center),
            )
            .padding(0)
            .style(flat_button(text_color))
            .on_press(toggle.clone())
            .into();
            let handle = match &handle_target {
                Some(target) => announcing_hoverable(
                    handle,
                    target.clone(),
                    handle_announcement,
                    GroupEvent::Tooltip,
                ),
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
                .extend((reveal > 0.0).then(|| {
                    revealed_content(config, reveal, items_row(config, &items, reveal, clock))
                }))
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
    clock: Clock,
) -> Element<'a, Outer> {
    let row = Row::with_children(items.iter().map(|item| item.view(config, 1.0, clock)))
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
pub(super) struct EdgeIndicator {
    pub highest_alert_severity: Severity,
    pub cursor_hovering: bool,
    pub expanded: bool,
}

impl EdgeIndicator {
    pub(super) fn view<'a, M: 'a>(self, config: &Config, reveal: f32) -> Element<'a, M> {
        let thickness = config.panel_bar.indicator_thickness;
        let base = if self.cursor_hovering { 0.5 } else { 0.2 };
        let revealing_opacity = base + (1.0 - base) * reveal;
        match self.highest_alert_severity {
            Severity::Low | Severity::Normal if self.expanded => {
                activity_line(primary_color, revealing_opacity, thickness)
            }
            Severity::Low | Severity::Normal if self.cursor_hovering => {
                activity_line(secondary_text_color, 1.0, thickness)
            }
            Severity::Low | Severity::Normal => Space::new()
                .width(Length::Fill)
                .height(Length::Fixed(thickness))
                .into(),
            severity @ (Severity::Warning | Severity::Critical) => {
                let tint = StateTint {
                    state: WidgetState::from(severity),
                    opacity: revealing_opacity,
                };
                activity_line(move |theme| tint.color(theme), 1.0, thickness)
            }
        }
    }
}

fn items_row<'a, M: Clone + 'a>(
    config: &Config,
    items: &[GroupItem<'a, M>],
    reveal: f32,
    clock: Clock,
) -> Element<'a, GroupEvent<M>> {
    Row::with_children(items.iter().map(|item| item.view(config, reveal, clock)))
        .spacing(config.layout.panel.widget_spacing)
        .align_y(Alignment::Center)
        .into()
}

fn collapsing_previewed_value<'a, M: 'a, Message: 'a>(
    config: &Config,
    items: &[GroupItem<'a, M>],
    reveal: f32,
    clock: Clock,
) -> Element<'a, Message> {
    let hidden = 1.0 - reveal;
    let previewed_widget = items
        .iter()
        .find_map(GroupItem::handle_preview)
        .map(|preview| preview.widget);
    let Some(value) = previewed_widget
        .filter(|_| hidden > 0.0)
        .and_then(|widget| {
            reading_value(
                config,
                widget,
                StateTint::opaque(widget.state(clock)),
                clock,
            )
        })
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
