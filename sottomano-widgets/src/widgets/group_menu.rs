mod cascading_columns;
mod menu_row;
mod quantity_table;
mod satellite_panels;

use std::{iter::once, rc::Rc};

use chrono::Locale;
use iced::{
    Element,
    widget::{Column, container},
};
use sottomano_config::{Config, screen_edges::Position, severity::Severity};

use self::{
    cascading_columns::{
        GroupMenuGroupRow, GroupMenuRowContext, cascading_columns, duplicate_group_titles,
        merged_where_requested,
    },
    quantity_table::{QuantityTableColumn, quantity_table},
    satellite_panels::CascadeSatellitePanels,
};
use crate::{
    clock::Clock,
    hover_cards::{
        GroupHandle, HoverAnnouncement, HoverTarget, InspectedRow, PopupMessage, PressHints,
        announcing_hoverable, hover_card_content::HoverCardContent,
    },
    widget_groups::group_menus::{
        AlertPromotedGroup, GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
    },
    widgets::{
        cascade_row::CascadeOrientation,
        configured_theme::text_color,
        hover_card_body::hover_card_source_delivery::HoverCard,
        icon_glyph::IconGlyph,
        pager_slot::PagerSlot,
        shell_layer_appearance::LayerAppearance,
        widget_group::{
            EdgeIndicator, GroupEvent, GroupMenuRowParts, GroupToastRequests, WidgetGroupContent,
            WidgetGroupTitle,
        },
        window_icon::WindowAssets,
    },
};

pub struct GroupMenuView<'a, M> {
    title: &'static str,
    glyph: IconGlyph,
    group_menus: &'a GroupMenuState,
    messenger: GroupMenuMessenger<'a, M>,
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
    fn view(&self, config: &Config) -> Option<Element<'static, PopupMessage>> {
        (!self.hover_card.is_empty())
            .then(|| HoverCardContent::new(self.hover_card, config, self.assets, self.clock).view())
    }
}

pub(super) struct GroupMenuMessenger<'a, M> {
    topic: GroupMenuTopic,
    on_message: Rc<dyn Fn(GroupMenuMessage) -> M + 'a>,
}

impl<M> GroupMenuMessenger<'_, M> {
    fn sent(&self, message: GroupMenuMessage) -> GroupEvent<M> {
        GroupEvent::Group((self.on_message)(message))
    }

    pub(super) fn toggle_pin(&self, entry: GroupMenuPin) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::TogglePin {
            menu: self.topic,
            entry,
        })
    }

    pub(super) fn inspect_member(&self, member: usize) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::InspectMember {
            menu: self.topic,
            member,
        })
    }

    pub(super) fn inspect_row(&self, row: InspectedRow) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::InspectRow {
            menu: self.topic,
            row,
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
            title_claim,
            face,
            aligned,
            item_rows,
        } = member;
        if !aligned.is_empty() {
            self.table_columns.push(QuantityTableColumn {
                group_title: title_claim.group_title.clone(),
                aligned,
            });
        }
        if !item_rows.is_empty() {
            self.group_rows.push(GroupMenuGroupRow {
                title_claim,
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
        on_message: impl Fn(GroupMenuMessage) -> M + 'a,
        groups: impl IntoIterator<Item = WidgetGroupContent<'a, M>>,
    ) -> Self {
        Self {
            title: topic.title(),
            glyph: topic.glyph().into(),
            group_menus,
            messenger: GroupMenuMessenger {
                topic,
                on_message: Rc::new(on_message),
            },
            groups: groups.into_iter().collect(),
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
            .filter(move |member| member.highest_alert_severity(clock) >= promotion_severity)
            .map(move |member| AlertPromotedGroup {
                menu,
                group_title: member.group_title(),
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
            self.glyph.view(config.theme.font.icon_size_px, text_color),
            HoverTarget::GroupHandle(GroupHandle::GroupMenu(self.messenger.topic)),
            HoverAnnouncement::from_group_handle(
                WidgetGroupTitle::fixed(self.title),
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
        PagerSlot::new(handle, indicator)
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
        let promotion_severity = config.panel_bar.group_menu_promotion_severity;
        let topic = self.messenger.topic;
        let group_menus = self.group_menus;
        once(Some(handle_slot)).chain(self.groups.into_iter().flat_map(move |member| {
            let group_entry = GroupMenuPin::Group {
                group_title: member.group_title(),
            };
            if group_menus.is_pinned(topic, group_entry) {
                return vec![
                    member
                        .pinned_from(topic)
                        .view(config, panel_position, clock),
                ];
            }
            if member.highest_alert_severity(clock) >= promotion_severity {
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
        let duplicate_titles =
            duplicate_group_titles(groups.iter().filter_map(WidgetGroupContent::title_claim));
        if !duplicate_titles.is_empty() {
            tracing::error!(
                "{} menu has groups sharing a title without requesting it: {duplicate_titles:?}",
                messenger.topic.title()
            );
            debug_assert!(
                duplicate_titles.is_empty(),
                "duplicate group titles: {duplicate_titles:?}"
            );
        }
        let MenuSections {
            table_columns,
            group_rows,
        } = groups
            .into_iter()
            .filter(|member| {
                let group_entry = GroupMenuPin::Group {
                    group_title: member.group_title(),
                };
                !group_menus.is_pinned(messenger.topic, group_entry)
            })
            .filter_map(|member| {
                member.menu_member(config, card.clock, group_menus, messenger.topic)
            })
            .fold(MenuSections::empty(), MenuSections::with_member);
        let table = quantity_table(table_columns, config).map(|table| {
            Element::from(container(table).padding(config.layout.overlay.content_inset))
                .map(Outer::from)
        });
        let satellite_panels = CascadeSatellitePanels::new(appearance, config, orientation);
        let cascade = cascading_columns(
            merged_where_requested(group_rows),
            &GroupMenuRowContext {
                config,
                direction: orientation.direction,
                menu: &messenger,
                group_menus,
                satellite_panels: &satellite_panels,
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
