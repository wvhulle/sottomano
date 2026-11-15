use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    slice,
};

use chrono::Locale;
use corredo::{
    gauges::activity_line,
    orientation::Axis,
    popovers::{NestedMenuOpenTrigger, NestedMenuRowEmphasis},
    press_actions::flat_button,
    theming::{primary_color, secondary_text_color, text_color},
    transitions::{expand_transition, reveal},
};
use iced::{
    Alignment, Element, Length, Padding,
    mouse::ScrollDelta,
    widget::{Row, Space, button, container, mouse_area},
};
use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot::PanelSlot,
    group_items::{
        aligned_quantity::ItemAlignment,
        group_item::{
            GroupItem, GroupItemHoverArea, GroupItemValue, MenuParts, RowHoverCard, RowScroll,
        },
        group_item_gestures::GroupItemGestures,
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        source_reading::{GroupItemIcon, reading_value},
    },
    group_menus::{
        group_menu_state::{
            AlertPromotion, GroupMenuPin, GroupMenuState, GroupMenuTopic, ItemPin, PanelPin,
            PanelPinDuration, PanelRaise,
        },
        group_menu_view::MenuSections,
        menu_rows::{
            group_menu_group_rows::GroupMenuGroupRow, quantity_table::QuantityTableColumn,
        },
    },
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverOrigin, HoverTarget, MenuHandleIndicator,
        PanelPinOrigin, PointerPresence, PressHints,
        hover_card_body::hover_card_source_delivery::SectionSource, hoverable,
    },
    widget_groups::widget_group_identity::{
        WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle,
    },
    widgets::{
        announced_level::LevelReading, icon_glyph::IconGlyph, ongoing_toast::OngoingToast,
        shared_metrics::ACCENT_STROKE_PX,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum WidgetGroupExpansionMessage {
    Toggle,
    Expand,
    CursorEntered,
    CursorExited,
    Collapse,
    HandleScrolled(ScrollDelta),
}

use corredo::transitions::ExpansionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpansionChange {
    JustCollapsed,
    Unchanged,
}

#[derive(Default)]
pub struct WidgetGroupExpansion {
    state: ExpansionState,
    cursor_pointing: PointerPresence,
}

pub struct KeyedExpansions<Key> {
    expansions: BTreeMap<Key, WidgetGroupExpansion>,
}

impl<Key> Default for KeyedExpansions<Key> {
    fn default() -> Self {
        Self {
            expansions: BTreeMap::new(),
        }
    }
}

impl<Key: Ord> KeyedExpansions<Key> {
    pub fn collapse(&mut self) {
        self.expansions.values_mut().for_each(|expansion| {
            expansion.collapse();
        });
    }

    #[must_use]
    pub fn combined_state(&self) -> ExpansionState {
        self.expansions
            .values()
            .map(WidgetGroupExpansion::state)
            .find(|expansion_state| *expansion_state == ExpansionState::Expanded)
            .unwrap_or(ExpansionState::Collapsed)
    }

    #[must_use]
    pub fn get(&self, key: &Key) -> Option<&WidgetGroupExpansion> {
        self.expansions.get(key)
    }

    pub fn update(&mut self, key: &Key, message: WidgetGroupExpansionMessage) {
        if let Some(expansion) = self.expansions.get_mut(key) {
            expansion.update(message);
        }
    }

    pub fn clear(&mut self) {
        self.expansions.clear();
    }

    pub fn track(&mut self, keys: impl IntoIterator<Item = Key>) {
        let current: BTreeSet<Key> = keys.into_iter().collect();
        self.expansions.retain(|key, _| current.contains(key));
        for key in current {
            self.expansions.entry(key).or_default();
        }
    }
}

impl WidgetGroupExpansion {
    #[must_use]
    pub fn handle_indicator(&self) -> MenuHandleIndicator {
        MenuHandleIndicator {
            pointing: self.cursor_pointing,
            menu_expansion: self.state,
        }
    }

    pub fn update(&mut self, msg: WidgetGroupExpansionMessage) -> ExpansionChange {
        let previous_state = self.state;
        match msg {
            WidgetGroupExpansionMessage::Toggle => {
                self.state = match self.state {
                    ExpansionState::Expanded => ExpansionState::Collapsed,
                    ExpansionState::Collapsed => ExpansionState::Expanded,
                }
            }
            WidgetGroupExpansionMessage::Expand => self.state = ExpansionState::Expanded,
            WidgetGroupExpansionMessage::CursorEntered => {
                self.cursor_pointing = PointerPresence::PointedAt;
            }
            WidgetGroupExpansionMessage::CursorExited => {
                self.cursor_pointing = PointerPresence::NotPointedAt;
            }
            WidgetGroupExpansionMessage::Collapse => self.state = ExpansionState::Collapsed,
            WidgetGroupExpansionMessage::HandleScrolled(_) => {}
        }
        match (previous_state, self.state) {
            (ExpansionState::Expanded, ExpansionState::Collapsed) => ExpansionChange::JustCollapsed,
            _ => ExpansionChange::Unchanged,
        }
    }

    pub fn collapse(&mut self) -> ExpansionChange {
        self.update(WidgetGroupExpansionMessage::Collapse)
    }

    #[must_use]
    pub fn state(&self) -> ExpansionState {
        self.state
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

pub enum WidgetGroupFace {
    Glyph {
        glyph: IconGlyph,
        state: ReadingDisplayState,
    },
    FirstItem,
}

impl WidgetGroupFace {
    fn hover_target(&self, group_identity: &WidgetGroupIdentity) -> Option<HoverTarget> {
        match self {
            WidgetGroupFace::Glyph { .. } => {
                Some(HoverTarget::GroupFace(Box::new(group_identity.clone())))
            }
            WidgetGroupFace::FirstItem => None,
        }
    }

    fn glyph_face_item<'a, M: Clone + 'a>(
        glyph: IconGlyph,
        state: ReadingDisplayState,
        group_identity: &WidgetGroupIdentity,
        group_title: &WidgetGroupTitle,
    ) -> GroupItem<'a, M> {
        GroupItem::composed(
            group_title.to_string(),
            GroupItemIcon::IconGlyph { glyph, state },
            GroupItemHoverArea::ByChildren {
                row_target: HoverTarget::GroupFace(Box::new(group_identity.clone())),
                child_targets: Vec::new(),
            },
            GroupItemValue::Nothing,
            Box::new(|_| Vec::new()),
        )
    }

    fn into_face_item<'a, M: Clone + 'a>(
        self,
        group_identity: &WidgetGroupIdentity,
        group_title: &WidgetGroupTitle,
        items: &mut Vec<GroupItem<'a, M>>,
        handle_announcement: HoverAnnouncement,
    ) -> GroupItem<'a, M> {
        match self {
            WidgetGroupFace::Glyph { glyph, state } => {
                Self::glyph_face_item(glyph, state, group_identity, group_title)
            }
            WidgetGroupFace::FirstItem => items.remove(0).acting_as_handle(handle_announcement),
        }
    }

    fn into_menu_parts<'a, M: Clone + 'a>(
        self,
        group_identity: &WidgetGroupIdentity,
        group_title: &WidgetGroupTitle,
        config: &'a Config,
        items: &[GroupItem<'a, M>],
        clock: Clock,
    ) -> MenuParts<'a, M> {
        match self {
            WidgetGroupFace::Glyph { glyph, state } => MenuParts {
                icon: Self::glyph_face_item(glyph, state, group_identity, group_title)
                    .menu_parts(config, clock)
                    .icon,
                value: collapsing_previewed_value(config, items, 0.0),
            },
            WidgetGroupFace::FirstItem => items[0].menu_parts(config, clock),
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

pub(crate) struct GroupMenuItemRow<'a, M> {
    pub name: Cow<'static, str>,
    pub pin: ItemPin,
    pub inspected_target: HoverTarget,
    pub gestures: GroupItemGestures<M>,
    pub row_scroll: RowScroll<'a, M>,
    pub parts: MenuParts<'a, M>,
}

impl<M> GroupMenuItemRow<'_, M> {
    pub(crate) fn row_emphasis(
        &self,
        inspected_target: Option<&HoverTarget>,
    ) -> NestedMenuRowEmphasis {
        if inspected_target == Some(&self.inspected_target) {
            NestedMenuRowEmphasis::Selected
        } else {
            NestedMenuRowEmphasis::Plain
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardTitleClash {
    pub item_name: String,
    pub clashing_title: String,
}

pub struct WidgetGroupContent<'a, M> {
    group_identity: WidgetGroupIdentity,
    group_title: WidgetGroupTitle,
    expansion: &'a WidgetGroupExpansion,
    on_expansion: Rc<dyn Fn(WidgetGroupExpansionMessage) -> M + 'a>,
    face: WidgetGroupFace,
    scroll_does: Option<&'static str>,
    items: Vec<GroupItem<'a, M>>,
    panel_pin: PanelPinOrigin,
}

impl<'a, M: Clone + 'a> WidgetGroupContent<'a, M> {
    pub fn expandable(
        family: WidgetGroupFamily,
        expansion: &'a WidgetGroupExpansion,
        on_expansion: impl Fn(WidgetGroupExpansionMessage) -> M + 'a,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> Self {
        Self::expandable_instance(
            WidgetGroupIdentity::sole(family),
            WidgetGroupTitle::fixed(family.title()),
            expansion,
            on_expansion,
            items,
        )
    }

    pub fn expandable_instance(
        group_identity: WidgetGroupIdentity,
        group_title: WidgetGroupTitle,
        expansion: &'a WidgetGroupExpansion,
        on_expansion: impl Fn(WidgetGroupExpansionMessage) -> M + 'a,
        items: impl IntoIterator<Item = GroupItem<'a, M>>,
    ) -> Self {
        Self {
            group_identity,
            group_title,
            expansion,
            on_expansion: Rc::new(on_expansion),
            face: WidgetGroupFace::FirstItem,
            scroll_does: None,
            items: items.into_iter().collect(),
            panel_pin: PanelPinOrigin::Unpinned,
        }
    }

    #[must_use]
    pub fn card_title_clashes(&self, clock: Clock) -> Vec<CardTitleClash> {
        let group_title = self.group_title.to_string();
        self.items
            .iter()
            .flat_map(|item| {
                let item_name = item.item_in_group_name().into_owned();
                let group_title = group_title.clone();
                item.hover_card(clock)
                    .into_iter()
                    .map(|section| {
                        let source_name = match section.source() {
                            SectionSource::Snapshot { source_name, .. } => source_name.to_owned(),
                            SectionSource::Local => String::new(),
                        };
                        (section.title().text().to_owned(), source_name)
                    })
                    .filter(|(section_title, source_name)| {
                        section_title.is_empty()
                            || *section_title == item_name
                            || *section_title == group_title
                            || section_title == source_name
                    })
                    .map(|(section_title, _)| CardTitleClash {
                        item_name: item_name.clone(),
                        clashing_title: section_title,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[must_use]
    pub fn clashing_item_names(&self) -> Vec<String> {
        let group_title = self.group_title.to_string();
        let mut seen_names = BTreeSet::new();
        self.items
            .iter()
            .filter(|item| item.menu_pin(&self.group_identity).is_some())
            .map(|item| item.item_in_group_name().into_owned())
            .filter(|item_name| *item_name == group_title || !seen_names.insert(item_name.clone()))
            .collect()
    }

    pub(crate) fn panel_raises(
        &self,
        menu: GroupMenuTopic,
        promotion_severity: Severity,
    ) -> Vec<PanelRaise> {
        self.items
            .iter()
            .filter(|item| item.alert_promotion(promotion_severity) == AlertPromotion::Promoted)
            .filter_map(|item| item.menu_pin(&self.group_identity))
            .map(|entry| PanelRaise { menu, entry })
            .collect()
    }

    #[must_use]
    pub fn pinned_from(self, menu: GroupMenuTopic) -> Self {
        Self {
            panel_pin: PanelPinOrigin::Pinned(Box::new(PanelPin {
                menu,
                entry: GroupMenuPin::Group(self.group_identity.clone()),
                duration: PanelPinDuration::Permanent,
            })),
            ..self
        }
    }

    #[must_use]
    pub fn face(self, face: WidgetGroupFace) -> Self {
        Self { face, ..self }
    }

    #[must_use]
    pub fn forwards_scroll(self, scroll_does: Option<&'static str>) -> Self {
        Self {
            scroll_does,
            ..self
        }
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
            group_identity: self.group_identity,
            group_title: self.group_title,
            expansion: self.expansion,
            on_expansion: {
                let on_expansion = self.on_expansion;
                Rc::new(move |message| lift(on_expansion(message)))
            },
            face: self.face,
            scroll_does: self.scroll_does,
            items: self.items.into_iter().map(|item| item.map(lift)).collect(),
            panel_pin: self.panel_pin,
        }
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests<M> {
        let expand = (self.on_expansion)(WidgetGroupExpansionMessage::Expand);
        self.items
            .iter()
            .map(|item| item.toast_requests(expand.clone(), locale, clock))
            .collect()
    }

    pub(crate) fn highest_alert_severity(&self) -> Severity {
        self.items
            .iter()
            .filter_map(GroupItem::shown_alert_severity)
            .max()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn row_hover_card(&self, target: &HoverTarget, clock: Clock) -> Option<RowHoverCard> {
        self.items
            .iter()
            .find(|item| item.owns_target(target))
            .map(|item| item.row_hover_card(clock))
    }

    pub(crate) fn group_identity(&self) -> &WidgetGroupIdentity {
        &self.group_identity
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub(crate) fn has_face(&self) -> bool {
        match self.face {
            WidgetGroupFace::Glyph { .. } => true,
            WidgetGroupFace::FirstItem => !self.items.is_empty(),
        }
    }

    #[cfg(test)]
    pub(crate) fn group_title(&self) -> WidgetGroupTitle {
        self.group_title.clone()
    }

    pub(crate) fn panel_item_views(
        self,
        group_menus: &GroupMenuState,
        menu: GroupMenuTopic,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Vec<Element<'a, M>>
    where
        M: From<GroupEvent<M>>,
    {
        let WidgetGroupContent {
            group_identity,
            group_title,
            items,
            ..
        } = self;
        let promotion_severity = config.group_menus.alert_promotion_severity;
        items
            .into_iter()
            .filter_map(|item| {
                let ItemPin::Pinned(panel_pin) =
                    group_menus.item_pin(menu, &group_identity, &item, promotion_severity)
                else {
                    return None;
                };
                let pinned_announcement = HoverAnnouncement {
                    press_hints: PressHints::default(),
                    origin: HoverOrigin::GroupHandle {
                        item_group_name: Box::new(group_title.clone()),
                        panel_pin: PanelPinOrigin::Pinned(Box::new(panel_pin)),
                    },
                };
                let item = item.acting_as_handle(pinned_announcement);
                Some(
                    fixed_view(
                        slice::from_ref(&item),
                        item.shown_alert_severity().unwrap_or_default(),
                        MenuHandleIndicator::default(),
                        config,
                        panel_position,
                        clock,
                    )
                    .map(M::from),
                )
            })
            .collect()
    }

    pub(crate) fn extend_menu_sections(
        self,
        config: &'a Config,
        clock: Clock,
        group_menus: &GroupMenuState,
        menu: GroupMenuTopic,
        mut sections: MenuSections<'a, M>,
    ) -> MenuSections<'a, M> {
        let WidgetGroupContent {
            group_identity,
            group_title,
            face,
            items,
            ..
        } = self;
        let face = face.into_menu_parts(&group_identity, &group_title, config, &items, clock);
        let aligned = items.iter().fold(BTreeMap::new(), |mut aligned, item| {
            if let ItemAlignment::Aligned(quantity) = item.alignment() {
                aligned.insert(quantity, item.table_cell_view(config, clock));
            }
            aligned
        });
        let item_rows: Vec<_> = items
            .iter()
            .filter(|item| item.alignment() == ItemAlignment::Unaligned)
            .map(|item| GroupMenuItemRow {
                name: item.item_in_group_name(),
                pin: group_menus.item_pin(
                    menu,
                    &group_identity,
                    item,
                    config.group_menus.alert_promotion_severity,
                ),
                inspected_target: item.inspected_target().clone(),
                gestures: item.gestures(),
                row_scroll: item.row_scroll(),
                parts: item.item_row_parts(config, clock),
            })
            .collect();
        if !aligned.is_empty() {
            sections.table_columns.push(QuantityTableColumn {
                group_title: group_title.clone(),
                aligned,
            });
        }
        if !item_rows.is_empty() {
            sections.group_rows.push(GroupMenuGroupRow {
                group_identity,
                group_title,
                face,
                items: item_rows,
            });
        }
        sections
    }

    #[must_use]
    pub fn view(
        self,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Option<Element<'a, M>>
    where
        M: From<GroupEvent<M>>,
    {
        self.view_events(config, panel_position, clock)
            .map(|element| element.map(M::from))
    }

    #[must_use]
    pub fn view_events(
        self,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Option<Element<'a, GroupEvent<M>>> {
        if self.items.is_empty() {
            return None;
        }
        let highest_alert_severity = self.highest_alert_severity();
        let WidgetGroupContent {
            group_identity,
            group_title,
            expansion,
            on_expansion,
            face,
            scroll_does,
            mut items,
            panel_pin,
            ..
        } = self;
        let handle_announcement = HoverAnnouncement {
            press_hints: PressHints::group_handle(expansion.state, scroll_does),
            origin: HoverOrigin::GroupHandle {
                item_group_name: Box::new(group_title.clone()),
                panel_pin,
            },
        };
        let face_target = face.hover_target(&group_identity);
        let face = face.into_face_item(
            &group_identity,
            &group_title,
            &mut items,
            handle_announcement.clone(),
        );
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
            handle: expansion.handle_indicator(),
        };
        let expansion_duration = config.widget_group_layout.expansion_duration;
        let animated = expand_transition(expansion.state, expansion_duration, move |reveal| {
            let handle: Element<'a, GroupEvent<M>> = button(
                Row::with_children([
                    draw_face(),
                    collapsing_previewed_value(config, &items, reveal),
                ])
                .align_y(Alignment::Center),
            )
            .padding(0)
            .style(flat_button(text_color))
            .on_press(toggle.clone())
            .into();
            let handle = match &handle_target {
                Some(target) => hoverable(
                    NestedMenuOpenTrigger::Hover,
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
            PanelSlot::new(inner.into(), edge_indicator.view(reveal)).view(panel_position)
        });
        Some(Element::from(
            mouse_area(animated).on_enter(on_enter).on_exit(on_exit),
        ))
    }
}

pub(crate) fn fixed_view<'a, M: Clone + 'a>(
    items: &[GroupItem<'a, M>],
    highest_alert_severity: Severity,
    handle_indicator: MenuHandleIndicator,
    config: &'a Config,
    panel_position: Position,
    clock: Clock,
) -> Element<'a, GroupEvent<M>> {
    let row = Row::with_children(items.iter().map(|item| item.view(config, 1.0, clock)))
        .spacing(config.theme.spacing.item_gap)
        .align_y(Alignment::Center);
    let indicator = EdgeIndicator::menu_handle_view(highest_alert_severity, handle_indicator);
    PanelSlot::new(row.into(), indicator).view(panel_position)
}

#[derive(Clone, Copy)]
pub(crate) struct EdgeIndicator {
    pub highest_alert_severity: Severity,
    pub handle: MenuHandleIndicator,
}

impl EdgeIndicator {
    pub(crate) fn menu_handle_view<'a, M: 'a>(
        highest_alert_severity: Severity,
        handle_indicator: MenuHandleIndicator,
    ) -> Element<'a, M> {
        let reveal = match handle_indicator.menu_expansion {
            ExpansionState::Expanded => 1.0,
            ExpansionState::Collapsed => 0.0,
        };
        Self {
            highest_alert_severity,
            handle: handle_indicator,
        }
        .view(reveal)
    }

    pub(crate) fn view<'a, M: 'a>(self, reveal: f32) -> Element<'a, M> {
        let thickness = ACCENT_STROKE_PX;
        let base = match self.handle.pointing {
            PointerPresence::PointedAt => 0.5,
            PointerPresence::NotPointedAt => 0.2,
        };
        let revealing_opacity = base + (1.0 - base) * reveal;
        match self.highest_alert_severity {
            Severity::Low | Severity::Normal
                if self.handle.menu_expansion == ExpansionState::Expanded =>
            {
                activity_line(
                    Axis::Horizontal,
                    primary_color,
                    revealing_opacity,
                    thickness,
                )
            }
            Severity::Low | Severity::Normal
                if self.handle.pointing == PointerPresence::PointedAt =>
            {
                activity_line(Axis::Horizontal, secondary_text_color, 1.0, thickness)
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
                activity_line(
                    Axis::Horizontal,
                    move |theme| tint.color(theme),
                    1.0,
                    thickness,
                )
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
        .spacing(config.theme.spacing.item_gap)
        .align_y(Alignment::Center)
        .into()
}

fn collapsing_previewed_value<'a, M: 'a, Message: 'a>(
    config: &Config,
    items: &[GroupItem<'a, M>],
    reveal_fraction: f32,
) -> Element<'a, Message> {
    let hidden = 1.0 - reveal_fraction;
    let Some(value) = items
        .iter()
        .find_map(GroupItem::handle_preview)
        .filter(|_| hidden > 0.0)
        .and_then(|preview| reading_value(config, preview.face, preview.readout, 1.0))
    else {
        return Row::new().into();
    };
    reveal(
        container(value).padding(Padding::ZERO.left(config.theme.spacing.item_gap)),
        hidden,
        Axis::Horizontal,
    )
    .into()
}

fn revealed_content<'a, Message: 'a>(
    config: &Config,
    reveal_fraction: f32,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let gap = config.theme.spacing.item_gap;
    reveal(
        container(content).padding(Padding::ZERO.left(gap)),
        reveal_fraction,
        Axis::Horizontal,
    )
    .into()
}

#[cfg(test)]
mod tests {
    use sottomano_toml_config::severity::Severity;

    use super::{WidgetGroupContent, WidgetGroupExpansion};
    use crate::{
        clock::Clock,
        group_items::{
            group_item::GroupItem, reading_display_state::ReadingDisplayState,
            reading_row::ReadingRow,
        },
        group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic, PanelRaise},
        hover_cards::HoverTarget,
        widget_groups::widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        widgets::icon_glyph::IconGlyph,
    };

    fn shown_state_row(shown_state: ReadingDisplayState) -> ReadingRow<'static> {
        ReadingRow::glyph_only(
            "Shown state reading",
            IconGlyph::default(),
            shown_state,
            Box::new(|_| Vec::new()),
        )
    }

    fn group_showing(
        expansion: &WidgetGroupExpansion,
        shown_state: ReadingDisplayState,
    ) -> WidgetGroupContent<'_, ()> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Weather,
            expansion,
            |_| (),
            [GroupItem::reading(
                shown_state_row(shown_state),
                HoverTarget::Workspace(7),
            )],
        )
    }

    #[test]
    fn a_critical_item_rises_onto_the_panel_without_a_toast() {
        let expansion = WidgetGroupExpansion::default();
        let group = group_showing(&expansion, ReadingDisplayState::Critical);
        assert_eq!(
            group.panel_raises(GroupMenuTopic::Media, Severity::Critical),
            vec![PanelRaise {
                menu: GroupMenuTopic::Media,
                entry: GroupMenuPin::Item {
                    group: WidgetGroupIdentity::sole(WidgetGroupFamily::Weather),
                    item_in_group_name: "Shown state reading".into(),
                },
            }]
        );
    }

    #[test]
    fn a_warning_item_stays_in_its_menu_below_a_critical_threshold() {
        let expansion = WidgetGroupExpansion::default();
        let group = group_showing(&expansion, ReadingDisplayState::Warning);
        assert_eq!(
            group.panel_raises(GroupMenuTopic::Media, Severity::Critical),
            Vec::new()
        );
        assert_eq!(group.highest_alert_severity(), Severity::Warning);
    }

    #[test]
    fn an_item_announcement_follows_the_current_press() {
        let expansion = WidgetGroupExpansion::default();
        let group_pressing = |description| {
            WidgetGroupContent::expandable(
                WidgetGroupFamily::NetworkManagerVpns,
                &expansion,
                |_| (),
                [GroupItem::reading(
                    shown_state_row(ReadingDisplayState::Idle),
                    HoverTarget::Workspace(7),
                )
                .on_press(description, ())],
            )
        };
        let left_hint = |description| {
            group_pressing(description)
                .row_hover_card(&HoverTarget::Workspace(7), Clock::real())
                .and_then(|row_card| row_card.announcement.press_hints.left)
        };
        assert_eq!(left_hint("connect the VPN"), Some("connect the VPN"));
        assert_eq!(left_hint("disconnect the VPN"), Some("disconnect the VPN"));
    }
}
