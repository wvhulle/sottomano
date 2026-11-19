use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use chrono::Locale;
use corredo::{
    gauges::activity_line,
    orientation::Axis,
    popovers::NestedMenuOpenTrigger,
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
    group_indicators::{
        aligned_quantity::IndicatorAlignment,
        group_indicator::{GroupIndicator, RowHoverCard},
        indicator_state::{IndicatorState, IndicatorStateTint},
    },
    group_menus::{
        group_menu_state::{
            AlertPromotion, GroupMenuPin, GroupMenuState, GroupMenuTopic, IndicatorPin, PanelPin,
            PanelPinDuration, PanelRaise,
        },
        group_menu_view::MenuSections,
        menu_rows::{
            group_menu_group_rows::GroupMenuGroupRow, quantity_table::QuantityTableColumn,
        },
    },
    hover_cards::{
        CardHeader, HoverCardMessage, HoverTarget, IndicatorKey, MenuHandleIndicator, MouseGesture,
        PanelPinOrigin, PointerPresence, PressAction,
        hover_card_body::{Section, hover_card_source_delivery::SectionSource},
        hoverable,
    },
    indicator_groups::indicator_group_identity::{
        IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
    },
    widgets::{
        announced_level::LevelToastRequest, ongoing_toast::OngoingToast,
        shared_metrics::ACCENT_STROKE_PX,
    },
};

#[derive(Debug, Clone, Copy)]
pub enum IndicatorGroupExpansionMessage {
    Toggle,
    Expand,
    CursorEntered,
    CursorExited,
    Collapse,
    HandleScrolled(ScrollDelta),
}

use corredo::transitions::ExpansionState;

use crate::{
    hover_cards::PopupMessage,
    indicator_groups::indicator_group_interaction::IndicatorGroupInteraction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpansionChange {
    JustCollapsed,
    Unchanged,
}

#[derive(Default)]
pub struct IndicatorGroupExpansion {
    pub state: ExpansionState,
    cursor_pointing: PointerPresence,
}

pub struct KeyedExpansions<Key> {
    pub expansions: BTreeMap<Key, IndicatorGroupExpansion>,
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
            .map(|expansion| expansion.state)
            .find(|expansion_state| *expansion_state == ExpansionState::Expanded)
            .unwrap_or(ExpansionState::Collapsed)
    }

    pub fn update(&mut self, key: &Key, message: IndicatorGroupExpansionMessage) {
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

impl IndicatorGroupExpansion {
    #[must_use]
    pub fn handle_indicator(&self) -> MenuHandleIndicator {
        MenuHandleIndicator {
            pointing: self.cursor_pointing,
            menu_expansion: self.state,
        }
    }

    pub fn update(&mut self, msg: IndicatorGroupExpansionMessage) -> ExpansionChange {
        let previous_state = self.state;
        match msg {
            IndicatorGroupExpansionMessage::Toggle => {
                self.state = match self.state {
                    ExpansionState::Expanded => ExpansionState::Collapsed,
                    ExpansionState::Collapsed => ExpansionState::Expanded,
                }
            }
            IndicatorGroupExpansionMessage::Expand => self.state = ExpansionState::Expanded,
            IndicatorGroupExpansionMessage::CursorEntered => {
                self.cursor_pointing = PointerPresence::PointedAt;
            }
            IndicatorGroupExpansionMessage::CursorExited => {
                self.cursor_pointing = PointerPresence::NotPointedAt;
            }
            IndicatorGroupExpansionMessage::Collapse => self.state = ExpansionState::Collapsed,
            IndicatorGroupExpansionMessage::HandleScrolled(_) => {}
        }
        match (previous_state, self.state) {
            (ExpansionState::Expanded, ExpansionState::Collapsed) => ExpansionChange::JustCollapsed,
            _ => ExpansionChange::Unchanged,
        }
    }

    pub fn collapse(&mut self) -> ExpansionChange {
        self.update(IndicatorGroupExpansionMessage::Collapse)
    }
}

#[derive(Debug, Clone)]
pub enum GroupEvent {
    Interaction(IndicatorGroupInteraction),
    Popup(PopupMessage),
}

impl From<HoverCardMessage> for GroupEvent {
    fn from(hover_card: HoverCardMessage) -> Self {
        GroupEvent::Popup(PopupMessage::HoverCard(hover_card))
    }
}

impl From<PopupMessage> for GroupEvent {
    fn from(popup: PopupMessage) -> Self {
        GroupEvent::Popup(popup)
    }
}

impl<Interaction: Into<IndicatorGroupInteraction>> From<Interaction> for GroupEvent {
    fn from(interaction: Interaction) -> Self {
        GroupEvent::Interaction(interaction.into())
    }
}

#[derive(Default)]
pub struct GroupToastRequests {
    pub ongoing_toasts: Vec<OngoingToast<IndicatorGroupInteraction>>,
    pub levels: Vec<LevelToastRequest<IndicatorGroupInteraction>>,
}

impl FromIterator<GroupToastRequests> for GroupToastRequests {
    fn from_iter<I: IntoIterator<Item = GroupToastRequests>>(groups: I) -> Self {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardTitleClash {
    pub indicator_name: String,
    pub clashing_title: String,
}

pub struct IndicatorGroupContent<'a> {
    pub(crate) group_identity: IndicatorGroupIdentity,
    pub(crate) group_title: IndicatorGroupTitle,
    expansion: &'a IndicatorGroupExpansion,
    on_expansion: Rc<dyn Fn(IndicatorGroupExpansionMessage) -> IndicatorGroupInteraction + 'a>,
    headline_indicator: GroupIndicator<'a>,
    scroll_does: Option<&'static str>,
    pub(crate) indicators: Vec<GroupIndicator<'a>>,
    panel_pin: PanelPinOrigin,
}

impl<'a> IndicatorGroupContent<'a> {
    pub fn expandable<Interaction: Into<IndicatorGroupInteraction>>(
        family: IndicatorGroupFamily,
        headline_indicator: GroupIndicator<'a>,
        expansion: &'a IndicatorGroupExpansion,
        on_expansion: impl Fn(IndicatorGroupExpansionMessage) -> Interaction + 'a,
        indicators: impl IntoIterator<Item = GroupIndicator<'a>>,
    ) -> Self {
        Self::expandable_instance(
            IndicatorGroupIdentity::sole(family),
            IndicatorGroupTitle::fixed(family.title()),
            headline_indicator,
            expansion,
            on_expansion,
            indicators,
        )
    }

    pub fn expandable_instance<Interaction: Into<IndicatorGroupInteraction>>(
        group_identity: IndicatorGroupIdentity,
        group_title: IndicatorGroupTitle,
        headline_indicator: GroupIndicator<'a>,
        expansion: &'a IndicatorGroupExpansion,
        on_expansion: impl Fn(IndicatorGroupExpansionMessage) -> Interaction + 'a,
        indicators: impl IntoIterator<Item = GroupIndicator<'a>>,
    ) -> Self {
        Self {
            group_identity,
            group_title,
            expansion,
            on_expansion: Rc::new(move |message| on_expansion(message).into()),
            headline_indicator,
            scroll_does: None,
            indicators: indicators.into_iter().collect(),
            panel_pin: PanelPinOrigin::Unpinned,
        }
    }

    #[must_use]
    pub fn card_title_clashes(&self, clock: Clock) -> Vec<CardTitleClash> {
        let group_title = self.group_title.to_string();
        self.indicators
            .iter()
            .flat_map(|indicator| {
                let indicator_name = indicator.indicator_name().into_owned();
                let group_title = group_title.clone();
                (indicator.card)(clock)
                    .into_iter()
                    .map(|section| {
                        let source_name = match section.source {
                            SectionSource::Snapshot { source_name, .. } => source_name.to_owned(),
                            SectionSource::Local => String::new(),
                        };
                        (section.title.text().to_owned(), source_name)
                    })
                    .filter(|(section_title, source_name)| {
                        section_title.is_empty()
                            || *section_title == indicator_name
                            || *section_title == group_title
                            || section_title == source_name
                    })
                    .map(|(section_title, _)| CardTitleClash {
                        indicator_name: indicator_name.clone(),
                        clashing_title: section_title,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[must_use]
    pub fn clashing_indicator_names(&self) -> Vec<String> {
        let group_title = self.group_title.to_string();
        let mut seen_names = BTreeSet::new();
        self.indicators
            .iter()
            .filter(|indicator| indicator.menu_pin(&self.group_identity).is_some())
            .map(|indicator| indicator.indicator_name().into_owned())
            .filter(|indicator_name| {
                *indicator_name == group_title || !seen_names.insert(indicator_name.clone())
            })
            .collect()
    }

    pub(crate) fn panel_raises(
        &self,
        menu: GroupMenuTopic,
        promotion_severity: Severity,
    ) -> Vec<PanelRaise> {
        self.indicators
            .iter()
            .filter(|indicator| {
                indicator.alert_promotion(promotion_severity) == AlertPromotion::Promoted
            })
            .filter_map(|indicator| indicator.menu_pin(&self.group_identity))
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
    pub fn forwards_scroll(self, scroll_does: Option<&'static str>) -> Self {
        Self {
            scroll_does,
            ..self
        }
    }

    #[must_use]
    pub fn only_aligned_indicators(self) -> Self {
        self.retaining_indicators(|alignment| alignment != IndicatorAlignment::Unaligned)
    }

    #[must_use]
    pub fn only_unaligned_indicators(self) -> Self {
        self.retaining_indicators(|alignment| alignment == IndicatorAlignment::Unaligned)
    }

    fn retaining_indicators(
        mut self,
        keeps_alignment: impl Fn(IndicatorAlignment) -> bool,
    ) -> Self {
        self.indicators
            .retain(|indicator| keeps_alignment(indicator.alignment));
        self
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests {
        let expand = (self.on_expansion)(IndicatorGroupExpansionMessage::Expand);
        self.indicators
            .iter()
            .map(|indicator| {
                indicator.toast_requests(&self.group_identity, expand.clone(), locale, clock)
            })
            .collect()
    }

    pub(crate) fn highest_alert_severity(&self) -> Severity {
        self.indicators
            .iter()
            .filter_map(GroupIndicator::shown_alert_severity)
            .max()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn indicator(&self, indicator_key: &IndicatorKey) -> Option<&GroupIndicator<'a>> {
        if indicator_key.group != self.group_identity {
            return None;
        }
        self.indicators
            .iter()
            .find(|indicator| indicator.indicator_name() == indicator_key.name)
    }

    #[must_use]
    pub fn indicator_keys(&self) -> Vec<IndicatorKey> {
        self.indicators
            .iter()
            .map(|indicator| indicator.indicator_key(&self.group_identity))
            .collect()
    }

    #[must_use]
    pub fn hovered_indicator(&self, target: &HoverTarget) -> Option<&GroupIndicator<'a>> {
        match target.headline_group() {
            Some(group_identity) if *group_identity == self.group_identity => {
                Some(&self.headline_indicator)
            }
            Some(_) => None,
            None => target
                .indicator_key()
                .and_then(|indicator_key| self.indicator(indicator_key)),
        }
    }

    fn handle_actions(&self) -> Vec<PressAction> {
        let face_actions = self.headline_indicator.press_actions();
        let handle_actions: Vec<PressAction> = [PressAction::group_handle(self.expansion.state)]
            .into_iter()
            .chain(
                self.scroll_does
                    .map(|scroll_does| PressAction::new(MouseGesture::Scroll, scroll_does)),
            )
            .filter(|handle_action| {
                face_actions
                    .iter()
                    .all(|face_action| face_action.gesture != handle_action.gesture)
            })
            .collect();
        face_actions.into_iter().chain(handle_actions).collect()
    }

    fn face_card(&self, clock: Clock) -> Vec<Section> {
        (self.headline_indicator.card)(clock)
            .into_iter()
            .chain(Section::press_actions(self.handle_actions()))
            .collect()
    }

    #[must_use]
    pub fn hover_card(&self, target: &HoverTarget, clock: Clock) -> Option<RowHoverCard> {
        let sections = match target.headline_group() {
            Some(group_identity) if *group_identity == self.group_identity => self.face_card(clock),
            Some(_) => return None,
            None => self
                .indicator(target.indicator_key()?)?
                .card_with_actions(clock),
        };
        let header = match target {
            HoverTarget::PinnedOnPanel(panel_pin) => CardHeader::Pinned {
                group_title: self.group_title.clone(),
                pin: panel_pin.clone(),
            },
            HoverTarget::Indicator(_)
            | HoverTarget::GroupHeadline(_)
            | HoverTarget::GroupMenu(_)
            | HoverTarget::SessionMenu
            | HoverTarget::Window(_)
            | HoverTarget::Workspace(_)
            | HoverTarget::WorkspaceOrder
            | HoverTarget::WorkspaceFolding => CardHeader::Unheaded,
        };
        Some(RowHoverCard { sections, header })
    }

    pub(crate) fn panel_indicator_views(
        self,
        group_menus: &GroupMenuState,
        menu: GroupMenuTopic,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Vec<Element<'a, GroupEvent>> {
        let promotion_severity = config.group_menus.alert_promotion_severity;
        self.indicators
            .iter()
            .filter_map(|indicator| {
                let IndicatorPin::Pinned(panel_pin) = group_menus.indicator_pin(
                    menu,
                    &self.group_identity,
                    indicator,
                    promotion_severity,
                ) else {
                    return None;
                };
                Some(fixed_view(
                    indicator,
                    HoverTarget::PinnedOnPanel(Box::new(panel_pin)),
                    indicator.shown_alert_severity().unwrap_or_default(),
                    MenuHandleIndicator::default(),
                    config,
                    panel_position,
                    clock,
                ))
            })
            .collect()
    }

    pub(crate) fn extend_menu_sections(
        self,
        config: &'a Config,
        clock: Clock,
        _group_menus: &GroupMenuState,
        _menu: GroupMenuTopic,
        mut sections: MenuSections<'a>,
    ) -> MenuSections<'a> {
        let IndicatorGroupContent {
            group_identity,
            group_title,
            headline_indicator,
            indicators,
            ..
        } = self;
        let (item_rows, aligned_indicators): (Vec<_>, Vec<_>) = indicators
            .into_iter()
            .partition(|indicator| indicator.alignment == IndicatorAlignment::Unaligned);
        let aligned = aligned_indicators
            .iter()
            .fold(BTreeMap::new(), |mut aligned, indicator| {
                if let IndicatorAlignment::Aligned(quantity) = indicator.alignment {
                    aligned.insert(quantity, indicator.table_cell_view(config, clock));
                }
                aligned
            });
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
                headline_indicator,
                indicators: item_rows,
            });
        }
        sections
    }

    #[must_use]
    pub fn view_events(
        self,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Option<Element<'a, GroupEvent>> {
        self.indicators.first()?;
        let highest_alert_severity = self.highest_alert_severity();
        let IndicatorGroupContent {
            group_identity,
            group_title: _,
            expansion,
            on_expansion,
            headline_indicator,
            scroll_does,
            indicators,
            panel_pin,
            ..
        } = self;
        let handle_target = match panel_pin {
            PanelPinOrigin::Pinned(panel_pin) => HoverTarget::PinnedOnPanel(panel_pin),
            PanelPinOrigin::Unpinned => {
                HoverTarget::GroupHeadline(Box::new(group_identity.clone()))
            }
        };
        let expansion_event = move |message| GroupEvent::Interaction(on_expansion(message));
        let toggle = expansion_event(IndicatorGroupExpansionMessage::Toggle);
        let on_enter = expansion_event(IndicatorGroupExpansionMessage::CursorEntered);
        let on_exit = expansion_event(IndicatorGroupExpansionMessage::CursorExited);
        let handle_indicator = expansion.handle_indicator();
        let expansion_duration = config.indicator_group_layout.expansion_duration;
        let animated = expand_transition(expansion.state, expansion_duration, move |reveal| {
            let handle: Element<'a, GroupEvent> =
                button(headline_indicator.handle_view(config, reveal))
                    .padding(0)
                    .style(flat_button(text_color))
                    .on_press(toggle.clone())
                    .into();
            let handle = hoverable(
                NestedMenuOpenTrigger::Hover,
                handle,
                handle_target.clone(),
                GroupEvent::from,
            );
            let handle = if scroll_does.is_some() {
                let expansion_event = expansion_event.clone();
                mouse_area(handle)
                    .on_scroll(move |delta| {
                        expansion_event(IndicatorGroupExpansionMessage::HandleScrolled(delta))
                    })
                    .into()
            } else {
                handle
            };
            let inner = Row::new()
                .push(handle)
                .extend((reveal > 0.0).then(|| {
                    revealed_content(
                        config,
                        reveal,
                        indicators_row(config, &group_identity, &indicators, reveal, clock),
                    )
                }))
                .align_y(Alignment::Center);
            PanelSlot::new(
                inner.into(),
                edge_indicator(highest_alert_severity, handle_indicator, reveal),
            )
            .view(panel_position)
        });
        Some(Element::from(
            mouse_area(animated).on_enter(on_enter).on_exit(on_exit),
        ))
    }
}

pub(crate) fn fixed_view<'a>(
    indicator: &GroupIndicator<'a>,
    hover_target: HoverTarget,
    highest_alert_severity: Severity,
    handle_indicator: MenuHandleIndicator,
    config: &'a Config,
    panel_position: Position,
    clock: Clock,
) -> Element<'a, GroupEvent> {
    let row = Row::with_children([indicator.view(config, 1.0, clock, hover_target)])
        .spacing(config.theme.spacing.item_gap)
        .align_y(Alignment::Center);
    let edge_indicator_view = edge_indicator(
        highest_alert_severity,
        handle_indicator,
        handle_indicator.settled_reveal(),
    );
    PanelSlot::new(row.into(), edge_indicator_view).view(panel_position)
}

pub(crate) fn edge_indicator<'a, Message: 'a>(
    highest_alert_severity: Severity,
    handle: MenuHandleIndicator,
    reveal: f32,
) -> Element<'a, Message> {
    let thickness = ACCENT_STROKE_PX;
    let base = match handle.pointing {
        PointerPresence::PointedAt => 0.5,
        PointerPresence::NotPointedAt => 0.2,
    };
    let revealing_opacity = base + (1.0 - base) * reveal;
    match highest_alert_severity {
        Severity::Low | Severity::Normal if handle.menu_expansion == ExpansionState::Expanded => {
            activity_line(
                Axis::Horizontal,
                primary_color,
                revealing_opacity,
                thickness,
            )
        }
        Severity::Low | Severity::Normal if handle.pointing == PointerPresence::PointedAt => {
            activity_line(Axis::Horizontal, secondary_text_color, 1.0, thickness)
        }
        Severity::Low | Severity::Normal => Space::new()
            .width(Length::Fill)
            .height(Length::Fixed(thickness))
            .into(),
        severity @ (Severity::Warning | Severity::Critical) => {
            let tint = IndicatorStateTint {
                state: IndicatorState::from(severity),
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

fn indicators_row<'r, 'a: 'r>(
    config: &Config,
    group_identity: &IndicatorGroupIdentity,
    rows: impl IntoIterator<Item = &'r GroupIndicator<'a>>,
    reveal: f32,
    clock: Clock,
) -> Element<'a, GroupEvent> {
    Row::with_children(rows.into_iter().map(|indicator| {
        indicator.view(
            config,
            reveal,
            clock,
            HoverTarget::Indicator(indicator.indicator_key(group_identity)),
        )
    }))
    .spacing(config.theme.spacing.item_gap)
    .align_y(Alignment::Center)
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
    use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

    use super::{IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage};
    use crate::{
        clock::Clock,
        glyph_icons::icon_glyph::IconGlyph,
        group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
        group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic, PanelRaise},
        hover_cards::{HoverTarget, IndicatorKey, MouseGesture, PressAction},
        indicator_groups::{
            clock_group::ClockInteraction,
            indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
        },
    };

    fn shown_state_indicator<'indicator>(
        shown_state: IndicatorState,
    ) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only("Shown state reading", IconGlyph::default(), shown_state)
            .with_hover_card(Box::new(|_| Vec::new()))
    }

    fn group_showing(
        expansion: &IndicatorGroupExpansion,
        shown_state: IndicatorState,
    ) -> IndicatorGroupContent<'_> {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Weather,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::Weather.title(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            expansion,
            ClockInteraction::Expansion,
            [shown_state_indicator(shown_state)],
        )
    }

    #[test]
    fn a_critical_indicator_rises_onto_the_panel_without_a_toast() {
        let expansion = IndicatorGroupExpansion::default();
        let group = group_showing(&expansion, IndicatorState::Critical);
        assert_eq!(
            group.panel_raises(GroupMenuTopic::Media, Severity::Critical),
            vec![PanelRaise {
                menu: GroupMenuTopic::Media,
                entry: GroupMenuPin::Item(IndicatorKey {
                    group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Weather),
                    name: "Shown state reading".into(),
                }),
            }]
        );
    }

    #[test]
    fn a_warning_indicator_stays_in_its_menu_below_a_critical_threshold() {
        let expansion = IndicatorGroupExpansion::default();
        let group = group_showing(&expansion, IndicatorState::Warning);
        assert_eq!(
            group.panel_raises(GroupMenuTopic::Media, Severity::Critical),
            Vec::new()
        );
        assert_eq!(group.highest_alert_severity(), Severity::Warning);
    }

    #[test]
    fn an_indicator_card_lists_the_current_press() {
        let expansion = IndicatorGroupExpansion::default();
        let group_pressing = |description| {
            IndicatorGroupContent::expandable(
                IndicatorGroupFamily::NetworkManagerVpns,
                GroupIndicator::glyph_only(
                    IndicatorGroupFamily::NetworkManagerVpns.title(),
                    IconGlyph::default(),
                    IndicatorState::Idle,
                ),
                &expansion,
                ClockInteraction::Expansion,
                [shown_state_indicator(IndicatorState::Idle).on_press(
                    description,
                    ClockInteraction::Expansion(IndicatorGroupExpansionMessage::Toggle),
                )],
            )
        };
        let indicator_key = IndicatorKey {
            group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::NetworkManagerVpns),
            name: "Shown state reading".into(),
        };
        let listed_actions = |description| {
            let group = group_pressing(description);
            assert!(
                group
                    .hover_card(
                        &HoverTarget::Indicator(indicator_key.clone()),
                        Clock::real()
                    )
                    .is_some()
            );
            group
                .indicator(&indicator_key)
                .map(GroupIndicator::press_actions)
        };
        assert_eq!(
            listed_actions("connect the VPN"),
            Some(vec![PressAction::new(
                MouseGesture::LeftClick,
                "connect the VPN"
            )])
        );
        assert_eq!(
            listed_actions("disconnect the VPN"),
            Some(vec![PressAction::new(
                MouseGesture::LeftClick,
                "disconnect the VPN"
            )])
        );
    }

    #[test]
    fn a_group_without_indicators_has_no_panel_view() {
        let expansion = IndicatorGroupExpansion::default();
        let config = Config::default();
        let empty_group = IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Weather,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::Weather.title(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            &expansion,
            ClockInteraction::Expansion,
            Vec::new(),
        );
        assert!(
            empty_group
                .view_events(&config, Position::Top, Clock::real())
                .is_none()
        );
    }
}
