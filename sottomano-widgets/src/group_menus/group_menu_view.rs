use std::iter::once;

use chrono::Locale;
use iced::{
    Element,
    widget::{Column, container},
};
use sottomano_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot::PanelSlot,
    group_menus::{
        cascading_layout::{
            cascade_row::CascadeOrientation,
            cascade_satellite_panels::CascadeSatellitePanels,
            cascading_columns::{
                GroupMenuGroupRow, GroupMenuRowContext, cascading_columns, one_row_per_identity,
            },
        },
        group_menu_state::{
            AlertPromotedGroup, GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
            OpenGroupMenu,
        },
        menu_rows::quantity_table::{QuantityTableColumn, quantity_table},
    },
    hover_cards::{
        HoverAnnouncement, HoverTarget, PopupMessage, PressHints, announcing_hoverable,
        hover_card_body::hover_card_source_delivery::HoverCard,
        hover_card_content::HoverCardContent,
    },
    widget_groups::{
        widget_group_content::{
            EdgeIndicator, GroupEvent, GroupMenuRowParts, GroupToastRequests, WidgetGroupContent,
        },
        widget_group_identity::{WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        configured_theme::text_color, icon_glyph::IconGlyph,
        shell_layer_appearance::LayerAppearance, window_icon::WindowAssets,
    },
};

pub struct GroupMenuView<'a, M> {
    group_menus: &'a GroupMenuState,
    messenger: GroupMenuMessenger<M>,
    groups: Vec<WidgetGroupContent<'a, M>>,
}

pub struct GroupMenuCascadeSurfaces<'a, Message> {
    pub primary: Element<'a, Message>,
    pub satellites: Vec<Element<'a, Message>>,
}

pub struct InspectionCard<'c, 'a> {
    pub hover_card: &'c HoverCard,
    pub assets: WindowAssets<'a>,
    pub clock: Clock,
}

impl InspectionCard<'_, '_> {
    pub(crate) fn view(&self, config: &Config) -> Option<Element<'static, PopupMessage>> {
        (!self.hover_card.is_empty())
            .then(|| HoverCardContent::new(self.hover_card, config, self.assets, self.clock).view())
    }
}

pub(crate) struct GroupMenuMessenger<M> {
    pub(crate) topic: GroupMenuTopic,
    on_message: fn(GroupMenuMessage) -> M,
}

impl<M> GroupMenuMessenger<M> {
    fn sent(&self, message: GroupMenuMessage) -> GroupEvent<M> {
        GroupEvent::Group((self.on_message)(message))
    }

    pub(crate) fn toggle_pin(&self, entry: GroupMenuPin) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::TogglePin {
            menu: self.topic,
            entry,
        })
    }

    pub(crate) fn reveal_group(&self, group_identity: WidgetGroupIdentity) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::RevealMenuGroup {
            menu: self.topic,
            group_identity,
        })
    }

    pub(crate) fn inspect_row(&self, target: HoverTarget) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::InspectRow {
            menu: self.topic,
            target,
        })
    }
}

struct MenuSections<'a, M> {
    table_columns: Vec<QuantityTableColumn<'a, M>>,
    group_rows: Vec<GroupMenuGroupRow<'a, M>>,
}

impl<'a, M> MenuSections<'a, M> {
    fn empty() -> Self {
        Self {
            table_columns: Vec::new(),
            group_rows: Vec::new(),
        }
    }

    fn with_member(mut self, member: GroupMenuRowParts<'a, M>) -> Self {
        let GroupMenuRowParts {
            group_identity,
            group_title,
            face,
            aligned,
            item_rows,
        } = member;
        if !aligned.is_empty() {
            self.table_columns.push(QuantityTableColumn {
                group_title: group_title.clone(),
                aligned,
            });
        }
        if !item_rows.is_empty() {
            self.group_rows.push(GroupMenuGroupRow {
                group_identity,
                group_title,
                face,
                items: item_rows,
            });
        }
        self
    }
}

impl<'a, M: Clone + 'a> GroupMenuView<'a, M> {
    pub fn new(
        topic: GroupMenuTopic,
        group_menus: &'a GroupMenuState,
        on_message: fn(GroupMenuMessage) -> M,
        groups: impl IntoIterator<Item = WidgetGroupContent<'a, M>>,
    ) -> Self {
        Self {
            group_menus,
            messenger: GroupMenuMessenger { topic, on_message },
            groups: groups.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn announcement_of(&self, target: &HoverTarget) -> Option<HoverAnnouncement> {
        self.groups
            .iter()
            .find_map(|group| group.item_announcement(target))
    }

    #[must_use]
    pub fn open_group_menu(&self) -> OpenGroupMenu {
        OpenGroupMenu {
            topic: self.messenger.topic,
            families: self
                .groups
                .iter()
                .map(|group| group.group_identity().family)
                .collect(),
        }
    }

    #[must_use]
    pub fn topic(&self) -> GroupMenuTopic {
        self.messenger.topic
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests<M> {
        self.groups
            .iter()
            .map(|member| member.toast_requests(locale, clock))
            .collect()
    }

    pub fn promoted_groups(
        &self,
        promotion_severity: Severity,
        clock: Clock,
    ) -> impl Iterator<Item = AlertPromotedGroup> {
        let menu = self.messenger.topic;
        self.groups
            .iter()
            .filter(move |member| member.promoted_by_alert(promotion_severity, clock))
            .map(move |member| AlertPromotedGroup {
                menu,
                group: member.group_identity().clone(),
            })
    }

    fn handle_view<Outer: From<GroupEvent<M>> + 'a>(
        &self,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Element<'a, Outer> {
        let highest_alert_severity = self
            .groups
            .iter()
            .map(|member| member.highest_alert_severity(clock))
            .max()
            .unwrap_or_default();
        let handle = announcing_hoverable(
            IconGlyph::from(self.messenger.topic.glyph())
                .view(config.theme.font.icon_size_px, text_color),
            HoverTarget::GroupMenu(self.messenger.topic),
            HoverAnnouncement::from_group_handle(
                WidgetGroupTitle::fixed(self.messenger.topic.title()),
                PressHints::default(),
            ),
            GroupEvent::HoverCard,
        );
        let indicator = EdgeIndicator {
            highest_alert_severity,
            cursor_hovering: false,
            expanded: false,
        }
        .view(config, 0.0);
        PanelSlot::new(handle, indicator)
            .view(config, panel_position)
            .map(Outer::from)
    }

    pub fn panel_views<Outer: From<GroupEvent<M>> + 'a>(
        self,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> impl Iterator<Item = Option<Element<'a, Outer>>> + 'a {
        let handle_slot = self.handle_view(config, panel_position, clock);
        let promotion_severity = config.group_menus.alert_promotion_severity;
        let topic = self.messenger.topic;
        let group_menus = self.group_menus;
        once(Some(handle_slot)).chain(self.groups.into_iter().flat_map(move |member| {
            let group_entry = GroupMenuPin::Group(member.group_identity().clone());
            if group_menus.is_pinned(topic, group_entry) {
                return vec![
                    member
                        .pinned_from(topic)
                        .view(config, panel_position, clock),
                ];
            }
            if member.promoted_by_alert(promotion_severity, clock) {
                return vec![member.view(config, panel_position, clock)];
            }
            member.pinned_item_views(group_menus, topic, config, panel_position, clock)
        }))
    }

    #[must_use]
    pub fn surfaces<Outer: From<GroupEvent<M>> + From<PopupMessage> + 'a>(
        self,
        config: &'a Config,
        appearance: &LayerAppearance,
        card: &InspectionCard<'_, 'a>,
        orientation: CascadeOrientation,
    ) -> Option<GroupMenuCascadeSurfaces<'a, Outer>> {
        let GroupMenuView {
            group_menus,
            messenger,
            groups,
            ..
        } = self;
        let MenuSections {
            table_columns,
            group_rows,
        } = groups
            .into_iter()
            .filter(|member| {
                let group_entry = GroupMenuPin::Group(member.group_identity().clone());
                !group_menus.is_pinned(messenger.topic, group_entry)
            })
            .filter_map(|member| {
                member.menu_member(config, card.clock, group_menus, messenger.topic)
            })
            .fold(MenuSections::empty(), MenuSections::with_member);
        let table = quantity_table(table_columns, config).map(|table| {
            Element::from(container(table).padding(config.overlay_surfaces.content_inset))
                .map(Outer::from)
        });
        let cascade_satellite_panels = CascadeSatellitePanels::new(appearance, config, orientation);
        let cascade = cascading_columns(
            one_row_per_identity(group_rows),
            &GroupMenuRowContext {
                config,
                direction: orientation.direction,
                menu: &messenger,
                group_menus,
                cascade_satellite_panels: &cascade_satellite_panels,
            },
            card,
        );
        match (table, cascade) {
            (None, None) => None,
            (Some(table), None) => Some(GroupMenuCascadeSurfaces {
                primary: table,
                satellites: Vec::new(),
            }),
            (None, Some(cascade)) => Some(cascade),
            (Some(table), Some(cascade)) => Some(GroupMenuCascadeSurfaces {
                primary: Column::with_children([table, cascade.primary]).into(),
                satellites: cascade.satellites,
            }),
        }
    }
}
