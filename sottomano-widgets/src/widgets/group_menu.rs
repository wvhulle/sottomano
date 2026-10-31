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
use sottomano_config::{Config, screen_edges::Position};

use self::{
    cascading_columns::{CascadeEntry, CascadeLayout, cascading_columns, merged_by_title},
    quantity_table::{QuantityTableColumn, quantity_table},
    satellite_panels::SatellitePanels,
};
use crate::{
    clock::Clock,
    hover_tooltips::{
        GroupHandle, HoverAnnouncement, HoverTarget, InspectedRow, PopupMessage, PressHints,
        announcing_hoverable, tooltip_content::TooltipContent,
    },
    widget_groups::group_menus::{GroupMenuKind, GroupMenuMessage, GroupMenus},
    widgets::{
        bar_widget::Glyph,
        cascade_row::CascadeOrientation,
        configured_theme::text_color,
        pager_slot::PagerSlot,
        shell_layer_appearance::LayerAppearance,
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::{EdgeIndicator, GroupContent, GroupEvent, GroupToastRequests, MenuMember},
        window_icon::WindowAssets,
    },
};

pub struct GroupMenu<'a, M> {
    title: &'static str,
    glyph: Glyph,
    group_menus: &'a GroupMenus,
    messenger: MenuMessenger<'a, M>,
    members: Vec<GroupContent<'a, M>>,
}

pub struct GroupMenuSurfaces<'a, Message> {
    pub primary: Element<'a, Message>,
    pub satellites: Vec<Element<'a, Message>>,
}

pub struct InspectionCard<'c, 'a> {
    pub tooltip: &'c Tooltip,
    pub assets: WindowAssets<'a>,
    pub clock: Clock,
}

impl InspectionCard<'_, '_> {
    fn view(&self, config: &Config) -> Option<Element<'static, PopupMessage>> {
        (!self.tooltip.is_empty())
            .then(|| TooltipContent::new(self.tooltip, config, self.assets, self.clock).view())
    }
}

pub(super) struct MenuMessenger<'a, M> {
    kind: GroupMenuKind,
    on_message: Rc<dyn Fn(GroupMenuMessage) -> M + 'a>,
}

impl<M> MenuMessenger<'_, M> {
    fn sent(&self, message: GroupMenuMessage) -> GroupEvent<M> {
        GroupEvent::Group((self.on_message)(message))
    }

    pub(super) fn toggle_pin(&self, group_title: &'static str) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::TogglePin {
            menu: self.kind,
            group_title,
        })
    }

    pub(super) fn inspect_member(&self, member: usize) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::InspectMember {
            menu: self.kind,
            member,
        })
    }

    pub(super) fn inspect_row(&self, row: InspectedRow) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::InspectRow {
            menu: self.kind,
            row,
        })
    }
}

struct MenuSections<'a, M> {
    table_columns: Vec<QuantityTableColumn<'a, M>>,
    cascade_entries: Vec<CascadeEntry<'a, M>>,
}

impl<'a, M> MenuSections<'a, M> {
    fn empty() -> Self {
        Self {
            table_columns: Vec::new(),
            cascade_entries: Vec::new(),
        }
    }

    fn with_member(mut self, member: MenuMember<'a, M>) -> Self {
        let MenuMember {
            group_title,
            face,
            aligned,
            cascade_items,
        } = member;
        if !aligned.is_empty() {
            self.table_columns.push(QuantityTableColumn {
                group_title,
                aligned,
            });
        }
        if !cascade_items.is_empty() {
            self.cascade_entries.push(CascadeEntry {
                group_title,
                face,
                items: cascade_items,
            });
        }
        self
    }
}

impl<'a, M: Clone + 'a> GroupMenu<'a, M> {
    pub fn new(
        kind: GroupMenuKind,
        title: &'static str,
        glyph: impl Into<Glyph>,
        group_menus: &'a GroupMenus,
        on_message: impl Fn(GroupMenuMessage) -> M + 'a,
        members: impl IntoIterator<Item = GroupContent<'a, M>>,
    ) -> Self {
        Self {
            title,
            glyph: glyph.into(),
            group_menus,
            messenger: MenuMessenger {
                kind,
                on_message: Rc::new(on_message),
            },
            members: members.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> GroupMenuKind {
        self.messenger.kind
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests<M> {
        self.members
            .iter()
            .map(|member| member.toast_requests(locale, clock))
            .collect()
    }

    fn handle_view<Outer: From<GroupEvent<M>> + 'a>(
        &self,
        config: &'a Config,
        bar_position: Position,
        clock: Clock,
    ) -> Element<'a, Outer> {
        let highest_alert_severity = self
            .members
            .iter()
            .map(|member| member.highest_alert_severity(clock))
            .max()
            .unwrap_or_default();
        let handle = announcing_hoverable(
            self.glyph.view(config.theme.font.icon_size_px, text_color),
            HoverTarget::GroupHandle(GroupHandle::GroupMenu(self.messenger.kind)),
            HoverAnnouncement::from_group_handle(self.title, PressHints::default()),
            GroupEvent::Tooltip,
        );
        let indicator = EdgeIndicator {
            highest_alert_severity,
            cursor_hovering: false,
            expanded: false,
        }
        .view(config, 0.0);
        PagerSlot::new(handle, indicator)
            .view(config, bar_position)
            .map(Outer::from)
    }

    pub fn bar_views<Outer: From<GroupEvent<M>> + 'a>(
        self,
        config: &'a Config,
        bar_position: Position,
        clock: Clock,
    ) -> impl Iterator<Item = Option<Element<'a, Outer>>> + 'a {
        let handle_slot = self.handle_view(config, bar_position, clock);
        let promotion_severity = config.panel_bar.group_menu_promotion_severity;
        let kind = self.messenger.kind;
        let group_menus = self.group_menus;
        once(Some(handle_slot)).chain(self.members.into_iter().filter_map(move |member| {
            if group_menus.is_pinned(kind, member.group_title()) {
                return Some(member.pinned_from(kind).view(config, bar_position, clock));
            }
            (member.highest_alert_severity(clock) >= promotion_severity)
                .then(|| member.view(config, bar_position, clock))
        }))
    }

    #[must_use]
    pub fn surfaces<Outer: From<GroupEvent<M>> + From<PopupMessage> + 'a>(
        self,
        config: &'a Config,
        appearance: &LayerAppearance,
        card: &InspectionCard<'_, 'a>,
        orientation: CascadeOrientation,
    ) -> Option<GroupMenuSurfaces<'a, Outer>> {
        let GroupMenu {
            group_menus,
            messenger,
            members,
            ..
        } = self;
        let MenuSections {
            table_columns,
            cascade_entries,
        } = members
            .into_iter()
            .filter(|member| !group_menus.is_pinned(messenger.kind, member.group_title()))
            .filter_map(|member| member.menu_member(config, card.clock))
            .fold(MenuSections::empty(), MenuSections::with_member);
        let table = quantity_table(table_columns, config).map(|table| {
            Element::from(container(table).padding(config.layout.overlay.content_inset))
                .map(Outer::from)
        });
        let satellite_panels = SatellitePanels::new(appearance, config, orientation);
        let cascade = cascading_columns(
            merged_by_title(cascade_entries),
            &CascadeLayout {
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
            (Some(table), None) => Some(GroupMenuSurfaces {
                primary: table,
                satellites: Vec::new(),
            }),
            (None, Some(cascade)) => Some(cascade),
            (Some(table), Some(cascade)) => Some(GroupMenuSurfaces {
                primary: Column::with_children([table, cascade.primary]).into(),
                satellites: cascade.satellites,
            }),
        }
    }
}
