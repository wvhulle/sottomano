use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::{Display, Formatter, Result as FmtResult},
    rc::Rc,
    slice,
};

use chrono::Locale;
use iced::{
    Alignment, Element, Length, Padding,
    mouse::ScrollDelta,
    widget::{Row, Space, button, container, mouse_area},
};
use sottomano_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot::PanelSlot,
    group_items::{
        aligned_quantity::{AlignedQuantity, ItemAlignment},
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
            MenuItemAction, MenuParts,
        },
        group_item_reading::GroupItemReading,
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        source_reading::{icon_value_gap, reading_value},
    },
    group_menus::group_menu_state::{GroupMenuState, GroupMenuTopic},
    hover_cards::{
        GroupHandle, HoverAnnouncement, HoverCardMessage, HoverTarget, PanelPinOrigin, PressHints,
        announcing_hoverable,
    },
    widget_groups::expand_animation::expand_animation,
    widgets::{
        activity_line::activity_line,
        announced_level::LevelReading,
        configured_theme::{primary_color, secondary_text_color, text_color},
        flat_button,
        icon_glyph::IconGlyph,
        ongoing_toast::OngoingToast,
        reveal::reveal_x,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetGroupTitle {
    Named(Cow<'static, str>),
    Numbered {
        base_title: Cow<'static, str>,
        ordinal: u8,
    },
}

const MOST_SIBLINGS_PER_TITLE: usize = 8;

impl WidgetGroupTitle {
    #[must_use]
    pub const fn fixed(title: &'static str) -> Self {
        WidgetGroupTitle::Named(Cow::Borrowed(title))
    }

    #[must_use]
    pub fn numbered_siblings(base_title: impl Into<Cow<'static, str>>) -> Vec<Self> {
        let base_title = base_title.into();
        (0..MOST_SIBLINGS_PER_TITLE)
            .map(|position| Self::numbered_from_second(base_title.clone(), position))
            .collect()
    }

    #[must_use]
    pub fn numbered_from_second(base_title: impl Into<Cow<'static, str>>, position: usize) -> Self {
        let base_title = base_title.into();
        match u8::try_from(position + 1) {
            Ok(ordinal) if ordinal > 1 => WidgetGroupTitle::Numbered {
                base_title,
                ordinal,
            },
            Ok(_) | Err(_) => WidgetGroupTitle::Named(base_title),
        }
    }
}

impl Display for WidgetGroupTitle {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            WidgetGroupTitle::Named(title) => formatter.write_str(title),
            WidgetGroupTitle::Numbered {
                base_title,
                ordinal,
            } => write!(formatter, "{base_title} {ordinal}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleRow {
    OwnRow,
    SharedWithSameTitle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupTitleClaim {
    pub group_title: WidgetGroupTitle,
    pub title_row: TitleRow,
}

#[derive(Debug, Clone, Copy)]
pub enum WidgetGroupExpansionMessage {
    Toggle,
    Expand,
    CursorEntered,
    CursorExited,
    Collapse,
    HandleScrolled(ScrollDelta),
}

#[derive(Default)]
pub struct WidgetGroupExpansion {
    expanded: bool,
    cursor_hovering: bool,
}

impl WidgetGroupExpansion {
    pub fn update(&mut self, msg: WidgetGroupExpansionMessage) -> bool {
        let was_expanded = self.is_expanded();
        match msg {
            WidgetGroupExpansionMessage::Toggle => self.expanded = !self.expanded,
            WidgetGroupExpansionMessage::Expand => self.expanded = true,
            WidgetGroupExpansionMessage::CursorEntered => self.cursor_hovering = true,
            WidgetGroupExpansionMessage::CursorExited => self.cursor_hovering = false,
            WidgetGroupExpansionMessage::Collapse => self.expanded = false,
            WidgetGroupExpansionMessage::HandleScrolled(_) => {}
        }
        was_expanded && !self.is_expanded()
    }

    pub fn collapse(&mut self) -> bool {
        self.update(WidgetGroupExpansionMessage::Collapse)
    }

    #[must_use]
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }
}

#[derive(Debug, Clone)]
pub enum GroupEvent<M> {
    Group(M),
    HoverCard(HoverCardMessage),
}

impl<M> GroupEvent<M> {
    pub fn map<Lifted>(self, lift: impl FnOnce(M) -> Lifted) -> GroupEvent<Lifted> {
        match self {
            GroupEvent::Group(message) => GroupEvent::Group(lift(message)),
            GroupEvent::HoverCard(hover_card) => GroupEvent::HoverCard(hover_card),
        }
    }
}

pub enum WidgetGroupFace<'a> {
    Icon {
        glyph: IconGlyph,
        handle: GroupHandle,
    },
    IconColoredBy {
        glyph: IconGlyph,
        reading: &'a dyn GroupItemReading,
        handle: GroupHandle,
    },
    FirstItem,
}

impl<'a> WidgetGroupFace<'a> {
    fn hover_target(&self) -> Option<HoverTarget> {
        match self {
            WidgetGroupFace::Icon { handle, .. }
            | WidgetGroupFace::IconColoredBy { handle, .. } => {
                Some(HoverTarget::GroupHandle(*handle))
            }
            WidgetGroupFace::FirstItem => None,
        }
    }

    fn into_face_item<M: Clone + 'a>(
        self,
        group_title: &WidgetGroupTitle,
        items: &mut Vec<GroupItem<'a, M>>,
        handle_announcement: HoverAnnouncement,
        clock: Clock,
    ) -> GroupItem<'a, M> {
        let glyph_face = |glyph, state, handle| {
            GroupItem::composed(
                ComposedGroupItemFace {
                    name: group_title.to_string().into(),
                    icon: GroupItemIcon::IconGlyph { glyph, state },
                },
                GroupItemHoverArea::ByChildren {
                    row_target: HoverTarget::GroupHandle(handle),
                },
                GroupItemValue::Nothing,
            )
        };
        match self {
            WidgetGroupFace::Icon { glyph, handle } => {
                glyph_face(glyph, ReadingDisplayState::Idle, handle)
            }
            WidgetGroupFace::IconColoredBy {
                glyph,
                reading,
                handle,
            } => glyph_face(glyph, reading.state(clock), handle),
            WidgetGroupFace::FirstItem => items.remove(0).acting_as_handle(handle_announcement),
        }
    }

    fn into_menu_parts<M: Clone + 'a>(
        self,
        config: &'a Config,
        items: &[GroupItem<'a, M>],
        clock: Clock,
    ) -> MenuParts<'a, M> {
        let preview = || collapsing_previewed_value(config, items, 0.0, clock);
        match self {
            WidgetGroupFace::Icon { glyph, .. } => MenuParts {
                icon: icon_face_glyph(config, glyph, FaceGlyphTint::Text),
                value: preview(),
            },
            WidgetGroupFace::IconColoredBy { glyph, reading, .. } => MenuParts {
                icon: icon_face_glyph(
                    config,
                    glyph,
                    FaceGlyphTint::ReadingTint(reading.state(clock)),
                ),
                value: preview(),
            },
            WidgetGroupFace::FirstItem => items[0].menu_parts(config, clock),
        }
    }
}

#[derive(Clone, Copy)]
enum FaceGlyphTint {
    Text,
    ReadingTint(ReadingDisplayState),
}

fn icon_face_glyph<'a, Message: 'a>(
    config: &Config,
    glyph: IconGlyph,
    tint: FaceGlyphTint,
) -> Element<'a, Message> {
    let icon_size = config.theme.font.icon_size_px;
    match tint {
        FaceGlyphTint::Text => glyph.view(icon_size, text_color),
        FaceGlyphTint::ReadingTint(state) => glyph.view(icon_size, move |theme| state.color(theme)),
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

enum WidgetGroupShape<'a, M> {
    Expandable {
        expansion: &'a WidgetGroupExpansion,
        on_expansion: Rc<dyn Fn(WidgetGroupExpansionMessage) -> M + 'a>,
        face: WidgetGroupFace<'a>,
        scroll_does: Option<&'static str>,
    },
    Fixed,
}

impl<'a, M: 'a> WidgetGroupShape<'a, M> {
    fn map<Outer: 'a>(self, lift: fn(M) -> Outer) -> WidgetGroupShape<'a, Outer> {
        match self {
            WidgetGroupShape::Expandable {
                expansion,
                on_expansion,
                face,
                scroll_does,
            } => WidgetGroupShape::Expandable {
                expansion,
                on_expansion: Rc::new(move |message| lift(on_expansion(message))),
                face,
                scroll_does,
            },
            WidgetGroupShape::Fixed => WidgetGroupShape::Fixed,
        }
    }

    fn expand_message(&self) -> Option<M> {
        match self {
            WidgetGroupShape::Expandable { on_expansion, .. } => {
                Some(on_expansion(WidgetGroupExpansionMessage::Expand))
            }
            WidgetGroupShape::Fixed => None,
        }
    }
}

pub(crate) struct GroupMenuRowParts<'a, M> {
    pub title_claim: GroupTitleClaim,
    pub face: MenuParts<'a, M>,
    pub aligned: BTreeMap<AlignedQuantity, Element<'a, GroupEvent<M>>>,
    pub item_rows: Vec<GroupMenuItemRow<'a, M>>,
}

pub(crate) enum GroupItemPin {
    Unpinnable,
    Pinnable(&'static str),
}

pub(crate) struct GroupMenuItemRow<'a, M> {
    pub name: Cow<'static, str>,
    pub pin: GroupItemPin,
    pub action: MenuItemAction<M>,
    pub parts: MenuParts<'a, M>,
}

pub trait WidgetGroup {
    const TITLE: &'static str;

    fn expandable_group<'a, M: Clone + 'a>(
        expansion: &'a WidgetGroupExpansion,
        on_expansion: impl Fn(WidgetGroupExpansionMessage) -> M + 'a,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> WidgetGroupContent<'a, M> {
        Self::expandable_group_titled(
            WidgetGroupTitle::fixed(Self::TITLE),
            expansion,
            on_expansion,
            items,
        )
    }

    fn expandable_group_titled<'a, M: Clone + 'a>(
        group_title: WidgetGroupTitle,
        expansion: &'a WidgetGroupExpansion,
        on_expansion: impl Fn(WidgetGroupExpansionMessage) -> M + 'a,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> WidgetGroupContent<'a, M> {
        WidgetGroupContent::new(group_title, expansion, on_expansion, items)
    }

    fn fixed_group<'a, M: Clone + 'a>(item: Option<GroupItem<'a, M>>) -> WidgetGroupContent<'a, M> {
        WidgetGroupContent::single(WidgetGroupTitle::fixed(Self::TITLE), item)
    }
}

pub struct WidgetGroupContent<'a, M> {
    group_title: WidgetGroupTitle,
    title_row: TitleRow,
    handle: WidgetGroupShape<'a, M>,
    items: Vec<GroupItem<'a, M>>,
    panel_pin: PanelPinOrigin,
}

impl<'a, M: Clone + 'a> WidgetGroupContent<'a, M> {
    fn new(
        group_title: WidgetGroupTitle,
        expansion: &'a WidgetGroupExpansion,
        on_expansion: impl Fn(WidgetGroupExpansionMessage) -> M + 'a,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> Self {
        Self {
            group_title,
            title_row: TitleRow::OwnRow,
            handle: WidgetGroupShape::Expandable {
                expansion,
                on_expansion: Rc::new(on_expansion),
                face: WidgetGroupFace::FirstItem,
                scroll_does: None,
            },
            items: items.into_iter().collect(),
            panel_pin: PanelPinOrigin::Unpinned,
        }
    }

    fn single(group_title: WidgetGroupTitle, item: Option<GroupItem<'a, M>>) -> Self {
        Self {
            group_title,
            title_row: TitleRow::OwnRow,
            handle: WidgetGroupShape::Fixed,
            items: item.into_iter().collect(),
            panel_pin: PanelPinOrigin::Unpinned,
        }
    }

    #[must_use]
    pub fn sharing_title_row(self) -> Self {
        Self {
            title_row: TitleRow::SharedWithSameTitle,
            ..self
        }
    }

    pub(crate) fn title_claim(&self) -> Option<GroupTitleClaim> {
        (!self.items.is_empty()).then(|| GroupTitleClaim {
            group_title: self.group_title.clone(),
            title_row: self.title_row,
        })
    }

    #[must_use]
    pub fn pinned_from(self, menu: GroupMenuTopic) -> Self {
        Self {
            panel_pin: PanelPinOrigin::GroupPinnedFrom(menu),
            ..self
        }
    }

    #[must_use]
    pub fn face(mut self, face: WidgetGroupFace<'a>) -> Self {
        if let WidgetGroupShape::Expandable { face: current, .. } = &mut self.handle {
            *current = face;
        }
        self
    }

    #[must_use]
    pub fn forwards_scroll(mut self, scroll_does: Option<&'static str>) -> Self {
        if let WidgetGroupShape::Expandable {
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

    pub fn map<Outer: Clone + 'a>(self, lift: fn(M) -> Outer) -> WidgetGroupContent<'a, Outer> {
        WidgetGroupContent {
            group_title: self.group_title,
            title_row: self.title_row,
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

    pub(crate) fn highest_alert_severity(&self, clock: Clock) -> Severity {
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

    pub(crate) fn group_title(&self) -> WidgetGroupTitle {
        self.group_title.clone()
    }

    pub(crate) fn pinned_item_views<Outer: From<GroupEvent<M>> + 'a>(
        self,
        group_menus: &GroupMenuState,
        menu: GroupMenuTopic,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Vec<Option<Element<'a, Outer>>> {
        let WidgetGroupContent {
            group_title, items, ..
        } = self;
        items
            .into_iter()
            .filter_map(|item| {
                let item_name = item.pinned_item_name(&group_title, group_menus, menu)?;
                let pinned_announcement = HoverAnnouncement::from_group_handle(
                    group_title.clone(),
                    PressHints::default(),
                )
                .pinned_by(PanelPinOrigin::ItemPinnedFrom { menu, item_name });
                let item = item.acting_as_handle(pinned_announcement);
                Some(fixed_view(
                    slice::from_ref(&item),
                    item.ongoing_alert_severity(clock).unwrap_or_default(),
                    config,
                    panel_position,
                    clock,
                ))
            })
            .map(Some)
            .collect()
    }

    pub(crate) fn menu_member(
        self,
        config: &'a Config,
        clock: Clock,
        group_menus: &GroupMenuState,
        menu: GroupMenuTopic,
    ) -> Option<GroupMenuRowParts<'a, M>> {
        if self.items.is_empty() {
            return None;
        }
        let WidgetGroupContent {
            group_title,
            title_row,
            handle,
            items,
            ..
        } = self;
        let face = match handle {
            WidgetGroupShape::Expandable { face, .. } => face,
            WidgetGroupShape::Fixed => WidgetGroupFace::FirstItem,
        }
        .into_menu_parts(config, &items, clock);
        let aligned = items.iter().fold(BTreeMap::new(), |mut aligned, item| {
            if let ItemAlignment::Aligned(quantity) = item.alignment() {
                aligned.insert(quantity, item.table_cell_view(config, clock));
            }
            aligned
        });
        let item_rows: Vec<_> = items
            .iter()
            .filter(|item| {
                item.alignment() == ItemAlignment::Unaligned
                    && item
                        .pinned_item_name(&group_title, group_menus, menu)
                        .is_none()
            })
            .map(|item| GroupMenuItemRow {
                name: item.menu_name(),
                pin: item
                    .pinnable_name()
                    .map_or(GroupItemPin::Unpinnable, GroupItemPin::Pinnable),
                action: item.menu_action(),
                parts: item.menu_parts(config, clock),
            })
            .collect();
        if aligned.is_empty() && item_rows.is_empty() {
            return None;
        }
        Some(GroupMenuRowParts {
            title_claim: GroupTitleClaim {
                group_title,
                title_row,
            },
            face,
            aligned,
            item_rows,
        })
    }

    pub fn view<Outer: From<GroupEvent<M>> + 'a>(
        self,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Option<Element<'a, Outer>> {
        if self.items.is_empty() {
            return None;
        }
        let highest_alert_severity = self.highest_alert_severity(clock);
        let WidgetGroupContent {
            group_title,
            handle,
            mut items,
            panel_pin,
            ..
        } = self;
        let WidgetGroupShape::Expandable {
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
                panel_position,
                clock,
            ));
        };
        let handle_announcement = HoverAnnouncement::from_group_handle(
            group_title.clone(),
            PressHints::group_handle(expansion.expanded, scroll_does),
        )
        .pinned_by(panel_pin);
        let face_target = face.hover_target();
        let face =
            face.into_face_item(&group_title, &mut items, handle_announcement.clone(), clock);
        let handle_target = items
            .iter()
            .find_map(GroupItem::handle_preview)
            .map(|preview| preview.target.clone())
            .or(face_target);
        let draw_face = move || face.view(config, 1.0, clock);
        let expansion_event = move |message| GroupEvent::Group(on_expansion(message));
        let toggle = expansion_event(WidgetGroupExpansionMessage::Toggle);
        let on_enter = expansion_event(WidgetGroupExpansionMessage::CursorEntered);
        let on_exit = expansion_event(WidgetGroupExpansionMessage::CursorExited);
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
                    handle_announcement.clone(),
                    GroupEvent::HoverCard,
                ),
                None => handle,
            };
            let handle = if scroll_does.is_some() {
                let expansion_event = expansion_event.clone();
                mouse_area(handle)
                    .on_scroll(move |delta| {
                        expansion_event(WidgetGroupExpansionMessage::HandleScrolled(delta))
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
            PanelSlot::new(inner.into(), edge_indicator.view(config, reveal))
                .view(config, panel_position)
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
    panel_position: Position,
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
    PanelSlot::new(row.into(), indicator)
        .view(config, panel_position)
        .map(Outer::from)
}

#[derive(Clone, Copy)]
pub(crate) struct EdgeIndicator {
    pub highest_alert_severity: Severity,
    pub cursor_hovering: bool,
    pub expanded: bool,
}

impl EdgeIndicator {
    pub(crate) fn view<'a, M: 'a>(self, config: &Config, reveal: f32) -> Element<'a, M> {
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
                let tint = ReadingStateTint {
                    state: ReadingDisplayState::from(severity),
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
                ReadingStateTint::opaque(widget.state(clock)),
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
