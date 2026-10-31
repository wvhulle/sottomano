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
    cascading_columns::{CascadeDevice, cascading_columns, merged_by_title},
    quantity_table::{QuantityTableColumn, quantity_table},
    satellite_panels::SatellitePanels,
};
use crate::{
    clock::Clock,
    hover_tooltips::{
        GroupHandle, HoverAnnouncement, HoverTarget, InspectedRow, PopupMessage, PressHints,
        announcing_hoverable, tooltip_content::TooltipContent,
    },
    widget_groups::group_menu_pins::GroupMenuPins,
    widgets::{
        bar_widget::Glyph,
        configured_theme::text_color,
        pager_slot::PagerSlot,
        shell_layer_appearance::LayerAppearance,
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::{EdgeIndicator, GroupContent, GroupEvent, GroupToastRequests, MenuMember},
        window_icon::WindowAssets,
    },
};

pub(super) type PinToggle<'a, M> = Rc<dyn Fn(&'static str) -> M + 'a>;

pub struct GroupMenu<'a, M> {
    handle: GroupHandle,
    group_title: &'static str,
    glyph: Glyph,
    pins: &'a GroupMenuPins,
    on_pin_toggle: PinToggle<'a, M>,
    members: Vec<GroupContent<'a, M>>,
}

pub struct MenuInspection<'i> {
    pub open_member: Option<usize>,
    pub inspected_row: Option<&'i InspectedRow>,
    pub card: &'i Tooltip,
}

impl<'a, M: Clone + 'a> GroupMenu<'a, M> {
    pub fn new(
        handle: GroupHandle,
        group_title: &'static str,
        glyph: impl Into<Glyph>,
        pins: &'a GroupMenuPins,
        on_pin_toggle: impl Fn(GroupHandle, &'static str) -> M + 'a,
        members: impl IntoIterator<Item = GroupContent<'a, M>>,
    ) -> Self {
        Self {
            handle,
            group_title,
            glyph: glyph.into(),
            pins,
            on_pin_toggle: Rc::new(move |member_title| on_pin_toggle(handle, member_title)),
            members: members.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn handle(&self) -> GroupHandle {
        self.handle
    }

    #[must_use]
    pub fn toast_requests(&self, locale: Locale, clock: Clock) -> GroupToastRequests<M> {
        self.members
            .iter()
            .map(|member| member.toast_requests(locale, clock))
            .collect()
    }

    pub fn handle_view<Outer: From<GroupEvent<M>> + 'a>(
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
            HoverTarget::GroupHandle(self.handle),
            HoverAnnouncement::from_group_handle(self.group_title, PressHints::default()),
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
        let GroupMenu {
            handle,
            pins,
            members,
            ..
        } = self;
        once(Some(handle_slot)).chain(members.into_iter().filter_map(move |member| {
            let member_title = member.group_title();
            if pins.is_pinned(handle, member_title) {
                return Some(member.pinned_from(handle).view(config, bar_position, clock));
            }
            (member.highest_alert_severity(clock) >= promotion_severity)
                .then(|| member.view(config, bar_position, clock))
        }))
    }

    #[must_use]
    pub fn surfaces<Outer: From<GroupEvent<M>> + From<PopupMessage> + 'a>(
        self,
        config: &'a Config,
        assets: WindowAssets<'a>,
        clock: Clock,
        appearance: &LayerAppearance,
        inspection: &MenuInspection<'_>,
    ) -> Option<GroupMenuSurfaces<'a, Outer>> {
        let satellite_panels = SatellitePanels::new(appearance, config);
        let GroupMenu {
            handle,
            pins,
            on_pin_toggle,
            members,
            ..
        } = self;
        let mut table_columns = Vec::new();
        let mut cascade_devices = Vec::new();
        for member in members
            .into_iter()
            .filter(|member| !pins.is_pinned(handle, member.group_title()))
            .filter_map(|member| member.menu_member(config, clock))
        {
            let MenuMember {
                group_title,
                face,
                aligned,
                cascade_items,
            } = member;
            if !aligned.is_empty() {
                table_columns.push(QuantityTableColumn {
                    group_title,
                    aligned,
                });
            }
            if !cascade_items.is_empty() {
                cascade_devices.push(CascadeDevice {
                    group_title,
                    face,
                    items: cascade_items,
                });
            }
        }
        let card = (!inspection.card.is_empty()).then(|| {
            Element::from(TooltipContent::new(inspection.card, config, assets, clock).view())
                .map(Outer::from)
        });
        let table = quantity_table(table_columns, config).map(|table| {
            Element::from(container(table).padding(config.layout.overlay.content_inset))
                .map(Outer::from)
        });
        let cascade = cascading_columns(
            merged_by_title(cascade_devices),
            &on_pin_toggle,
            config,
            &satellite_panels,
            inspection,
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

pub struct GroupMenuSurfaces<'a, Message> {
    pub primary: Element<'a, Message>,
    pub satellites: Vec<Element<'a, Message>>,
}
