use std::iter::once;

use chrono::Locale;
use corredo::{
    popovers::{NestedMenuAction, NestedMenuOpenTrigger},
    theming::text_color,
};
use iced::{Alignment, Element, widget::row};
use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot::PanelSlot,
    glyph_icons::{icon_glyph::IconGlyph, phosphor::Phosphor},
    group_indicators::group_indicator::{GroupIndicator, RowHoverCard},
    group_menus::{
        group_menu_state::{
            AlertPromotion, GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
            OpenGroupMenu, PanelRaise,
        },
        menu_rows::{
            group_menu_group_rows::GroupMenuGroupRow, quantity_table::QuantityTableColumn,
        },
    },
    hover_cards::{
        CardHeader, HoverTarget, IndicatorKey, MenuHandleIndicator, PanelPinOrigin, PopupMessage,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
        hover_card_content::HoverCardContent,
    },
    icon_catalog::{icon_handle::IconHandle, window_icons::WindowAssets},
    indicator_groups::{
        indicator_group_content::{
            GroupEvent, GroupToastRequests, IndicatorGroupContent, group_panel_view::edge_indicator,
        },
        indicator_group_identity::IndicatorGroupIdentity,
        indicator_group_interaction::IndicatorGroupInteraction,
    },
    widgets::shell_layer_appearance::LayerAppearance,
};

const BADGE_SCALE: f32 = 0.6;

mod group_menu_cascade;
#[cfg(test)]
mod group_menu_layout_tests;
#[cfg(test)]
mod inspection_card_tests;
#[cfg(test)]
mod row_pointer_tests;

pub fn panel_menu_handle(
    open_trigger: NestedMenuOpenTrigger,
    content: Element<'_, GroupEvent>,
    target: HoverTarget,
    highest_alert_severity: Severity,
    handle_indicator: MenuHandleIndicator,
    panel_position: Position,
) -> Element<'_, GroupEvent> {
    let edge_indicator_view = edge_indicator(
        highest_alert_severity,
        handle_indicator,
        handle_indicator.settled_reveal(),
    );
    PanelSlot::new(content, edge_indicator_view).hovered(
        panel_position,
        open_trigger,
        target,
        GroupEvent::from,
    )
}

pub struct GroupMenuView<'view> {
    group_menus: &'view GroupMenuState,
    messenger: GroupMenuMessenger,
    groups: Vec<IndicatorGroupContent<'view>>,
    handle_badge: MenuHandleBadge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MenuHandleBadge {
    Unbadged,
    Badged(Phosphor),
    ImageBadged(IconHandle),
}

pub struct InspectionCard<'sections, 'assets> {
    pub sections: &'sections [Section],
    pub card_appearance: LayerAppearance,
    pub assets: WindowAssets<'assets>,
    pub clock: Clock,
}

impl InspectionCard<'_, '_> {
    #[must_use]
    pub fn view(&self, config: &Config) -> Option<Element<'static, PopupMessage>> {
        let hover_card = self.described_hover_card()?;
        Some(
            HoverCardContent::new(&hover_card, config, self.assets, self.clock)
                .view(self.card_appearance.frame),
        )
    }

    fn described_hover_card(&self) -> Option<HoverCard> {
        HoverCard::new(self.sections.to_vec(), CardHeader::Unheaded)
    }
}

pub(crate) struct GroupMenuMessenger {
    pub(crate) topic: GroupMenuTopic,
}

pub(crate) fn menu_event(message: GroupMenuMessage) -> GroupEvent {
    GroupEvent::Interaction(IndicatorGroupInteraction::GroupMenus(message))
}

impl GroupMenuMessenger {
    pub(crate) fn toggle_pin(&self, entry: GroupMenuPin) -> GroupEvent {
        menu_event(GroupMenuMessage::TogglePin {
            menu: self.topic,
            entry,
        })
    }

    pub(crate) fn menu_action(
        &self,
    ) -> impl Fn(NestedMenuAction<IndicatorGroupIdentity>) -> GroupEvent + use<> {
        let menu = self.topic;
        move |action| menu_event(GroupMenuMessage::Menu { menu, action })
    }

    pub(crate) fn inspect_row(&self, indicator: IndicatorKey) -> GroupEvent {
        menu_event(GroupMenuMessage::InspectRow {
            menu: self.topic,
            indicator,
        })
    }
}

pub(crate) struct MenuSections<'view> {
    pub table_columns: Vec<QuantityTableColumn<'view>>,
    pub group_rows: Vec<GroupMenuGroupRow<'view>>,
}

impl MenuSections<'_> {
    fn empty() -> Self {
        Self {
            table_columns: Vec::new(),
            group_rows: Vec::new(),
        }
    }
}

impl<'view> GroupMenuView<'view> {
    pub fn new(
        topic: GroupMenuTopic,
        group_menus: &'view GroupMenuState,
        groups: impl IntoIterator<Item = IndicatorGroupContent<'view>>,
    ) -> Self {
        Self {
            group_menus,
            messenger: GroupMenuMessenger { topic },
            groups: groups.into_iter().collect(),
            handle_badge: MenuHandleBadge::Unbadged,
        }
    }

    #[must_use]
    pub fn with_handle_badge(mut self, handle_badge: MenuHandleBadge) -> Self {
        self.handle_badge = handle_badge;
        self
    }

    #[must_use]
    pub fn indicator(&self, indicator_key: &IndicatorKey) -> Option<&GroupIndicator<'view>> {
        self.groups
            .iter()
            .find_map(|group| group.indicator(indicator_key))
    }

    #[must_use]
    pub fn hovered_indicator(&self, target: &HoverTarget) -> Option<&GroupIndicator<'view>> {
        self.groups
            .iter()
            .find_map(|group| group.hovered_indicator(target))
    }

    #[must_use]
    pub fn hover_card(&self, target: &HoverTarget, clock: Clock) -> Option<RowHoverCard> {
        self.groups
            .iter()
            .find_map(|group| group.hover_card(target, clock))
    }

    #[must_use]
    pub fn indicator_hover_card(&self, indicator_key: &IndicatorKey, clock: Clock) -> Vec<Section> {
        self.indicator(indicator_key)
            .map(|indicator| indicator.card_with_actions(clock))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn open_group_menu(&self) -> OpenGroupMenu {
        OpenGroupMenu {
            topic: self.messenger.topic,
            families: self
                .groups
                .iter()
                .map(|group| group.group_identity.family)
                .collect(),
        }
    }

    #[must_use]
    pub fn topic(&self) -> GroupMenuTopic {
        self.messenger.topic
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests {
        self.groups
            .iter()
            .map(|member| member.toast_requests(locale, clock))
            .collect()
    }

    pub fn panel_raises(&self, promotion_severity: Severity) -> impl Iterator<Item = PanelRaise> {
        let menu = self.messenger.topic;
        self.groups
            .iter()
            .flat_map(move |member| member.panel_raises(menu, promotion_severity))
    }

    fn handle_view(
        &self,
        config: &'view Config,
        panel_position: Position,
        handle_indicator: MenuHandleIndicator,
    ) -> Element<'view, GroupEvent> {
        let highest_alert_severity = self
            .groups
            .iter()
            .map(IndicatorGroupContent::highest_alert_severity)
            .max()
            .unwrap_or_default();
        let icon_size = config.theme.font.icon_size_px;
        let topic_glyph = IconGlyph::from(self.messenger.topic.glyph()).view(icon_size, text_color);
        let glyph: Element<'view, GroupEvent> = match &self.handle_badge {
            MenuHandleBadge::Unbadged => topic_glyph,
            MenuHandleBadge::ImageBadged(badge_image) => {
                row![topic_glyph, badge_image.view(icon_size, 1.0)]
                    .spacing(icon_size * 0.25)
                    .align_y(Alignment::Center)
                    .into()
            }
            MenuHandleBadge::Badged(badge_glyph) => row![
                topic_glyph,
                IconGlyph::from(*badge_glyph).view(icon_size * BADGE_SCALE, text_color)
            ]
            .align_y(Alignment::End)
            .into(),
        };
        panel_menu_handle(
            self.group_menus.presentation.open_trigger,
            glyph,
            HoverTarget::GroupMenu(self.messenger.topic),
            highest_alert_severity,
            handle_indicator,
            panel_position,
        )
    }

    pub fn panel_views(
        self,
        config: &'view Config,
        panel_position: Position,
        handle_indicator: MenuHandleIndicator,
        clock: Clock,
    ) -> impl Iterator<Item = Element<'view, GroupEvent>> + 'view {
        let handle_slot = self.handle_view(config, panel_position, handle_indicator);
        let topic = self.messenger.topic;
        let group_menus = self.group_menus;
        once(handle_slot).chain(self.groups.into_iter().flat_map(move |member| {
            let group_entry = GroupMenuPin::Group(member.group_identity.clone());
            if let PanelPinOrigin::Pinned(_) =
                group_menus.panel_pin(topic, group_entry, AlertPromotion::Unpromoted)
            {
                return member
                    .pinned_from(topic)
                    .view_events(config, panel_position, clock)
                    .into_iter()
                    .collect();
            }
            member.panel_indicator_views(group_menus, topic, config, panel_position, clock)
        }))
    }
}
