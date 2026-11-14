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
        aligned_quantity::{AlignedQuantity, ItemAlignment},
        group_item::{
            ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
            MenuParts, RowScroll,
        },
        group_item_gestures::GroupItemGestures,
        group_item_reading::GroupItemReading,
        reading_display_state::{ReadingDisplayState, ReadingStateTint},
        source_reading::reading_value,
    },
    group_menus::group_menu_state::{
        AlertPinState, GroupMenuPin, GroupMenuState, GroupMenuTopic, MenuPinState,
        PanelPinDuration, PanelRaise,
    },
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverTarget, MenuHandleIndicator, PanelPinOrigin,
        PointerPresence, PressHints, announcing_hoverable,
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

pub enum WidgetGroupFace<'a> {
    Icon {
        glyph: IconGlyph,
    },
    IconColoredBy {
        glyph: IconGlyph,
        reading: &'a dyn GroupItemReading,
    },
    FirstItem,
}

impl<'a> WidgetGroupFace<'a> {
    fn hover_target(&self, group_identity: &WidgetGroupIdentity) -> Option<HoverTarget> {
        match self {
            WidgetGroupFace::Icon { .. } | WidgetGroupFace::IconColoredBy { .. } => {
                Some(HoverTarget::GroupFace(Box::new(group_identity.clone())))
            }
            WidgetGroupFace::FirstItem => None,
        }
    }

    fn into_face_item<M: Clone + 'a>(
        self,
        group_identity: &WidgetGroupIdentity,
        group_title: &WidgetGroupTitle,
        items: &mut Vec<GroupItem<'a, M>>,
        handle_announcement: HoverAnnouncement,
        clock: Clock,
    ) -> GroupItem<'a, M> {
        let glyph_face = |glyph, state| {
            GroupItem::composed(
                ComposedGroupItemFace {
                    name: group_title.to_string().into(),
                    icon: GroupItemIcon::IconGlyph { glyph, state },
                },
                GroupItemHoverArea::ByChildren {
                    row_target: HoverTarget::GroupFace(Box::new(group_identity.clone())),
                },
                GroupItemValue::Nothing,
            )
        };
        match self {
            WidgetGroupFace::Icon { glyph } => glyph_face(glyph, ReadingDisplayState::Idle),
            WidgetGroupFace::IconColoredBy { glyph, reading } => {
                glyph_face(glyph, reading.state(clock))
            }
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

pub(crate) struct GroupMenuRowParts<'a, M> {
    pub group_identity: WidgetGroupIdentity,
    pub group_title: WidgetGroupTitle,
    pub face: MenuParts<'a, M>,
    pub aligned: BTreeMap<AlignedQuantity, Element<'a, GroupEvent<M>>>,
    pub item_rows: Vec<GroupMenuItemRow<'a, M>>,
}

pub(crate) enum GroupItemPin {
    Unpinnable,
    Unpinned(String),
    Pinned(String),
}

pub(crate) struct GroupMenuItemRow<'a, M> {
    pub name: Cow<'static, str>,
    pub pin: GroupItemPin,
    pub inspected_target: HoverTarget,
    pub gestures: GroupItemGestures<M>,
    pub row_scroll: RowScroll<'a, M>,
    pub parts: MenuParts<'a, M>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowInspection {
    Inspected,
    Uninspected,
}

impl<M> GroupMenuItemRow<'_, M> {
    pub(crate) fn inspection(&self, inspected_target: Option<&HoverTarget>) -> RowInspection {
        if inspected_target == Some(&self.inspected_target) {
            RowInspection::Inspected
        } else {
            RowInspection::Uninspected
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPromotion {
    RisesToPanel,
    StaysInMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertPromotion {
    AlertingItemsOntoPanel,
    WholeGroupOntoPanel,
    StaysInMenu,
}

pub struct WidgetGroupContent<'a, M> {
    group_identity: WidgetGroupIdentity,
    group_title: WidgetGroupTitle,
    alert_promotion: AlertPromotion,
    expansion: &'a WidgetGroupExpansion,
    on_expansion: Rc<dyn Fn(WidgetGroupExpansionMessage) -> M + 'a>,
    face: WidgetGroupFace<'a>,
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
            alert_promotion: AlertPromotion::AlertingItemsOntoPanel,
            expansion,
            on_expansion: Rc::new(on_expansion),
            face: WidgetGroupFace::FirstItem,
            scroll_does: None,
            items: items.into_iter().collect(),
            panel_pin: PanelPinOrigin::Unpinned,
        }
    }

    #[must_use]
    pub fn staying_in_menu_during_alerts(self) -> Self {
        Self {
            alert_promotion: AlertPromotion::StaysInMenu,
            ..self
        }
    }

    #[must_use]
    pub fn with_panel_promotion(self, panel_promotion: PanelPromotion) -> Self {
        match panel_promotion {
            PanelPromotion::RisesToPanel => Self {
                alert_promotion: AlertPromotion::WholeGroupOntoPanel,
                ..self
            },
            PanelPromotion::StaysInMenu => self,
        }
    }

    pub(crate) fn whole_group_promotion(&self) -> PanelPromotion {
        match self.alert_promotion {
            AlertPromotion::WholeGroupOntoPanel => PanelPromotion::RisesToPanel,
            AlertPromotion::AlertingItemsOntoPanel | AlertPromotion::StaysInMenu => {
                PanelPromotion::StaysInMenu
            }
        }
    }

    pub(crate) fn panel_raises(
        &self,
        menu: GroupMenuTopic,
        promotion_severity: Severity,
        clock: Clock,
    ) -> Vec<PanelRaise> {
        match self.alert_promotion {
            AlertPromotion::WholeGroupOntoPanel => vec![PanelRaise::WholeGroup {
                menu,
                group: self.group_identity.clone(),
            }],
            AlertPromotion::AlertingItemsOntoPanel => self
                .items
                .iter()
                .filter(|item| item.alerts_from(promotion_severity, clock))
                .filter_map(GroupItem::pinnable_key)
                .map(|item_key| PanelRaise::AlertingItem {
                    menu,
                    group: self.group_identity.clone(),
                    item_key,
                })
                .collect(),
            AlertPromotion::StaysInMenu => Vec::new(),
        }
    }

    #[must_use]
    pub fn pinned_from(self, menu: GroupMenuTopic) -> Self {
        Self {
            panel_pin: PanelPinOrigin::GroupPinnedFrom {
                menu,
                group: Box::new(self.group_identity.clone()),
            },
            ..self
        }
    }

    #[must_use]
    pub fn face(self, face: WidgetGroupFace<'a>) -> Self {
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
            alert_promotion: self.alert_promotion,
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
        self.toast_requests_revealed_by(&expand, locale, clock)
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
        self.items
            .iter()
            .filter_map(|item| item.ongoing_alert_severity(clock))
            .max()
            .unwrap_or_default()
    }

    pub(crate) fn item_announcement(&self, target: &HoverTarget) -> Option<HoverAnnouncement> {
        self.items
            .iter()
            .find(|item| item.inspected_target() == target)
            .map(GroupItem::announcement)
    }

    pub(crate) fn group_identity(&self) -> &WidgetGroupIdentity {
        &self.group_identity
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
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
            alert_promotion,
            items,
            ..
        } = self;
        let promotion_severity = config.group_menus.alert_promotion_severity;
        items
            .into_iter()
            .filter_map(|item| {
                let item_key = item.pinnable_key()?;
                let entry = GroupMenuPin::Item {
                    group: group_identity.clone(),
                    item_key: item_key.clone(),
                };
                let pin_duration = match group_menus.pin_state(menu, entry.clone()) {
                    MenuPinState::Pinned => PanelPinDuration::Permanent,
                    MenuPinState::Unpinned
                        if alert_promotion == AlertPromotion::AlertingItemsOntoPanel
                            && item.alerts_from(promotion_severity, clock)
                            && group_menus.alert_pin_state(menu, entry)
                                == AlertPinState::Raised =>
                    {
                        PanelPinDuration::UntilAlertEnds
                    }
                    MenuPinState::Unpinned => return None,
                };
                let pinned_announcement = HoverAnnouncement::from_group_handle(
                    group_title.clone(),
                    PressHints::default(),
                )
                .pinned_by(PanelPinOrigin::ItemPinnedFrom {
                    menu,
                    group: Box::new(group_identity.clone()),
                    item_key,
                    pin_duration,
                });
                let item = item.acting_as_handle(pinned_announcement);
                Some(
                    fixed_view(
                        slice::from_ref(&item),
                        item.ongoing_alert_severity(clock).unwrap_or_default(),
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
            group_identity,
            group_title,
            face,
            items,
            ..
        } = self;
        let face = face.into_menu_parts(config, &items, clock);
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
                name: item.menu_name(),
                pin: match item.pinned_item_key(&group_identity, group_menus, menu) {
                    Some(pinned_key) => GroupItemPin::Pinned(pinned_key),
                    None => match item.pinnable_key() {
                        Some(unpinned_key) => GroupItemPin::Unpinned(unpinned_key),
                        None => GroupItemPin::Unpinnable,
                    },
                },
                inspected_target: item.inspected_target().clone(),
                gestures: item.gestures(),
                row_scroll: item.row_scroll(),
                parts: item.item_row_parts(config, clock),
            })
            .collect();
        if aligned.is_empty() && item_rows.is_empty() {
            return None;
        }
        Some(GroupMenuRowParts {
            group_identity,
            group_title,
            face,
            aligned,
            item_rows,
        })
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
        let highest_alert_severity = self.highest_alert_severity(clock);
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
        let handle_announcement = HoverAnnouncement::from_group_handle(
            group_title.clone(),
            PressHints::group_handle(expansion.state, scroll_does),
        )
        .pinned_by(panel_pin);
        let face_target = face.hover_target(&group_identity);
        let face = face.into_face_item(
            &group_identity,
            &group_title,
            &mut items,
            handle_announcement.clone(),
            clock,
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
            cursor_pointing: expansion.cursor_pointing,
            expansion_state: expansion.state,
        };
        let expansion_duration = config.widget_group_layout.expansion_duration;
        let animated = expand_transition(expansion.state, expansion_duration, move |reveal| {
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
    let indicator = EdgeIndicator::of_menu_handle(highest_alert_severity, handle_indicator);
    PanelSlot::new(row.into(), indicator).view(panel_position)
}

#[derive(Clone, Copy)]
pub(crate) struct EdgeIndicator {
    pub highest_alert_severity: Severity,
    pub cursor_pointing: PointerPresence,
    pub expansion_state: ExpansionState,
}

impl EdgeIndicator {
    pub(crate) fn of_menu_handle<'a, M: 'a>(
        highest_alert_severity: Severity,
        handle_indicator: MenuHandleIndicator,
    ) -> Element<'a, M> {
        let reveal = match handle_indicator.menu_expansion {
            ExpansionState::Expanded => 1.0,
            ExpansionState::Collapsed => 0.0,
        };
        Self {
            highest_alert_severity,
            cursor_pointing: handle_indicator.pointing,
            expansion_state: handle_indicator.menu_expansion,
        }
        .view(reveal)
    }

    pub(crate) fn view<'a, M: 'a>(self, reveal: f32) -> Element<'a, M> {
        let thickness = ACCENT_STROKE_PX;
        let base = match self.cursor_pointing {
            PointerPresence::PointedAt => 0.5,
            PointerPresence::NotPointedAt => 0.2,
        };
        let revealing_opacity = base + (1.0 - base) * reveal;
        match self.highest_alert_severity {
            Severity::Low | Severity::Normal
                if self.expansion_state == ExpansionState::Expanded =>
            {
                activity_line(
                    Axis::Horizontal,
                    primary_color,
                    revealing_opacity,
                    thickness,
                )
            }
            Severity::Low | Severity::Normal
                if self.cursor_pointing == PointerPresence::PointedAt =>
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
    clock: Clock,
) -> Element<'a, Message> {
    let hidden = 1.0 - reveal_fraction;
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
    use chrono::Locale;
    use sottomano_toml_config::severity::Severity;

    use super::{WidgetGroupContent, WidgetGroupExpansion};
    use crate::{
        clock::Clock,
        group_items::{
            group_item::GroupItem,
            group_item_reading::{GroupItemReading, OngoingToastThreshold},
            reading_display_state::ReadingDisplayState,
        },
        group_menus::group_menu_state::{GroupMenuTopic, PanelRaise},
        hover_cards::{HoverTarget, hover_card_body::HoverCardBody},
        widget_groups::{
            widget_group_content::PanelPromotion,
            widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        },
        widgets::{
            icon_glyph::IconGlyph,
            phosphor::{Phosphor, Weight},
        },
    };

    struct AlertingReading;

    impl GroupItemReading for AlertingReading {
        fn name(&self) -> &'static str {
            "Alerting reading"
        }

        fn icon(&self) -> IconGlyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn state(&self, _clock: Clock) -> ReadingDisplayState {
            ReadingDisplayState::Warning
        }

        fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
            OngoingToastThreshold::NotifyFromSeverity {
                minimum_severity: Severity::Warning,
                summary_text: "Alerting",
            }
        }

        fn ongoing_toast_body(&self, _clock: Clock, _locale: Locale) -> Option<String> {
            None
        }

        fn hover_card(&self, _clock: Clock) -> HoverCardBody {
            HoverCardBody::sections(Vec::new())
        }
    }

    fn alerting_group(expansion: &WidgetGroupExpansion) -> WidgetGroupContent<'_, ()> {
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Weather,
            expansion,
            |_| (),
            [GroupItem::reading(
                AlertingReading,
                HoverTarget::Workspace(7),
            )],
        )
    }

    #[test]
    fn an_alert_raises_only_its_item_onto_the_panel_by_default() {
        let expansion = WidgetGroupExpansion::default();
        let group = alerting_group(&expansion);
        assert_eq!(group.whole_group_promotion(), PanelPromotion::StaysInMenu);
        assert_eq!(
            group.panel_raises(GroupMenuTopic::Media, Severity::Warning, Clock::real()),
            vec![PanelRaise::AlertingItem {
                menu: GroupMenuTopic::Media,
                group: WidgetGroupIdentity::sole(WidgetGroupFamily::Weather),
                item_key: HoverTarget::Workspace(7).stable_item_key(),
            }]
        );
    }

    #[test]
    fn a_group_staying_in_its_menu_is_not_promoted_by_its_alert() {
        let expansion = WidgetGroupExpansion::default();
        let group = alerting_group(&expansion).staying_in_menu_during_alerts();
        assert_eq!(
            group.panel_raises(GroupMenuTopic::Media, Severity::Warning, Clock::real()),
            Vec::new()
        );
        assert_eq!(
            group.highest_alert_severity(Clock::real()),
            Severity::Warning
        );
    }

    #[test]
    fn an_item_announcement_follows_the_current_press() {
        let expansion = WidgetGroupExpansion::default();
        let group_pressing = |description| {
            WidgetGroupContent::expandable(
                WidgetGroupFamily::NetworkManagerVpns,
                &expansion,
                |_| (),
                [
                    GroupItem::reading(AlertingReading, HoverTarget::Workspace(7))
                        .on_press(description, ()),
                ],
            )
        };
        let left_hint = |description| {
            group_pressing(description)
                .item_announcement(&HoverTarget::Workspace(7))
                .and_then(|announcement| announcement.press_hints.left)
        };
        assert_eq!(left_hint("connect the VPN"), Some("connect the VPN"));
        assert_eq!(left_hint("disconnect the VPN"), Some("disconnect the VPN"));
    }
}
