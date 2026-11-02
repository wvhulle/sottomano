use std::iter::once;

use chrono::Locale;
use iced::{Element, widget::Column};
use sottomano_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot::PanelSlot,
    group_menus::{
        accordion_layout::accordion_sections::accordion_sections,
        cascading_layout::{
            cascade_row::CascadeOrientation, cascade_satellite_panels::CascadeSatellitePanels,
            cascading_columns::cascading_columns,
        },
        group_menu_presentation::GroupMenuLayout,
        group_menu_state::{
            AlertPromotedGroup, GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
            OpenGroupMenu,
        },
        menu_rows::{
            group_menu_group_rows::{GroupMenuGroupRow, GroupMenuRowContext, one_row_per_identity},
            quantity_table::{QuantityTableColumn, quantity_table},
        },
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

    pub(crate) fn toggle_group(&self, group_identity: WidgetGroupIdentity) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::ToggleMenuGroup {
            menu: self.topic,
            group_identity,
        })
    }

    pub(crate) fn scroll_message(&self) -> impl Fn(f32) -> GroupEvent<M> + use<M> {
        let on_message = self.on_message;
        let menu = self.topic;
        move |scrolled_px| {
            GroupEvent::Group(on_message(GroupMenuMessage::MenuScrolled {
                menu,
                scrolled_px,
            }))
        }
    }

    pub(crate) fn toggle_row_card(&self, target: HoverTarget) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::ToggleRowCard {
            menu: self.topic,
            target,
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

    fn handle_view(
        &self,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> Element<'a, M>
    where
        M: From<GroupEvent<M>>,
    {
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
            .map(M::from)
    }

    pub fn panel_views(
        self,
        config: &'a Config,
        panel_position: Position,
        clock: Clock,
    ) -> impl Iterator<Item = Element<'a, M>> + 'a
    where
        M: From<GroupEvent<M>>,
    {
        let handle_slot = self.handle_view(config, panel_position, clock);
        let promotion_severity = config.group_menus.alert_promotion_severity;
        let topic = self.messenger.topic;
        let group_menus = self.group_menus;
        once(handle_slot).chain(self.groups.into_iter().flat_map(move |member| {
            let group_entry = GroupMenuPin::Group(member.group_identity().clone());
            if group_menus.is_pinned(topic, group_entry) {
                return member
                    .pinned_from(topic)
                    .view(config, panel_position, clock)
                    .into_iter()
                    .collect();
            }
            if member.promoted_by_alert(promotion_severity, clock) {
                return member
                    .view(config, panel_position, clock)
                    .into_iter()
                    .collect();
            }
            member.pinned_item_views(group_menus, topic, config, panel_position, clock)
        }))
    }

    #[must_use]
    pub fn surfaces(
        self,
        config: &'a Config,
        appearance: &LayerAppearance,
        card: &InspectionCard<'_, 'a>,
        orientation: CascadeOrientation,
    ) -> Option<GroupMenuCascadeSurfaces<'a, M>>
    where
        M: From<GroupEvent<M>> + From<PopupMessage>,
    {
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
        let table = quantity_table(table_columns, config).map(|table| table.map(M::from));
        let cascade_satellite_panels = CascadeSatellitePanels::new(appearance, config, orientation);
        let row_context = GroupMenuRowContext {
            config,
            direction: orientation.direction,
            presentation: group_menus.presentation(),
            menu: &messenger,
            group_menus,
            cascade_satellite_panels: &cascade_satellite_panels,
        };
        let group_rows = one_row_per_identity(group_rows);
        let cascade = match row_context.presentation.layout {
            GroupMenuLayout::Cascading => cascading_columns(group_rows, &row_context, card),
            GroupMenuLayout::Accordion => accordion_sections(group_rows, &row_context, card),
        };
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

#[cfg(test)]
mod tests {
    use iced::{
        Settings, Size,
        widget::{Row, text},
    };
    use iced_controller_diagram::ControllerArtwork;
    use iced_test::Simulator;
    use sottomano_config::{Config, screen_edges::Position};

    use super::{GroupEvent, GroupMenuView, InspectionCard};
    use crate::{
        clock::Clock,
        desktop_index::DesktopIndex,
        group_items::{
            group_item::{ComposedGroupItemFace, GroupItem, GroupItemIcon, GroupItemValue},
            group_item_reading::GroupItemReading,
        },
        group_menus::{
            cascading_layout::{
                cascade_row::{CascadeDirection, CascadeOrientation},
                notched_panel::PanelGrowth,
            },
            group_menu_presentation::{
                GroupMenuLayout, GroupMenuPresentation, GroupRevealTrigger, HoverCardPlacement,
                OpenGroupLimit,
            },
            group_menu_state::{GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic},
        },
        hover_cards::{
            HoverTarget, PopupMessage,
            hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
        },
        widget_groups::{
            widget_group_content::{WidgetGroupContent, WidgetGroupExpansion},
            widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        },
        widgets::{
            favicon::Favicons,
            icon_glyph::IconGlyph,
            phosphor::{Phosphor, Weight},
            shell_layer_appearance::ShellLayer,
            window_icon::WindowAssets,
        },
    };

    #[derive(Debug, Clone)]
    struct SmokeMessage;

    impl From<GroupEvent<SmokeMessage>> for SmokeMessage {
        fn from(_: GroupEvent<SmokeMessage>) -> Self {
            SmokeMessage
        }
    }

    impl From<PopupMessage> for SmokeMessage {
        fn from(_: PopupMessage) -> Self {
            SmokeMessage
        }
    }

    struct SmokeReading;

    impl GroupItemReading for SmokeReading {
        fn name(&self) -> &'static str {
            "Smoke item"
        }

        fn icon(&self) -> IconGlyph {
            Phosphor::Question(Weight::Regular).into()
        }

        fn hover_card(&self, _clock: Clock) -> HoverCardBody {
            HoverCardBody::sections([])
        }
    }

    fn every_presentation() -> Vec<GroupMenuPresentation> {
        let layouts = [GroupMenuLayout::Cascading, GroupMenuLayout::Accordion];
        let triggers = [GroupRevealTrigger::Hover, GroupRevealTrigger::Press];
        let limits = [OpenGroupLimit::One, OpenGroupLimit::Several];
        let placements = [
            HoverCardPlacement::BesideRow,
            HoverCardPlacement::InlineWithRow,
        ];
        layouts
            .into_iter()
            .flat_map(|layout| {
                triggers
                    .into_iter()
                    .flat_map(move |cascading_reveal_trigger| {
                        limits.into_iter().flat_map(move |accordion_open_limit| {
                            placements.into_iter().map(move |hover_card_placement| {
                                GroupMenuPresentation {
                                    layout,
                                    cascading_reveal_trigger,
                                    accordion_open_limit,
                                    hover_card_placement,
                                }
                            })
                        })
                    })
            })
            .collect()
    }

    fn opened_menu_state(presentation: GroupMenuPresentation) -> GroupMenuState {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(presentation));
        let _ = group_menus.update(GroupMenuMessage::RevealMenuGroup {
            menu: GroupMenuTopic::Hardware,
            group_identity: WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu),
        });
        group_menus
    }

    fn row_tops(
        config: &Config,
        group_menus: &GroupMenuState,
        orientation: CascadeOrientation,
        clock: Clock,
    ) -> (f32, Option<f32>) {
        let expansion = WidgetGroupExpansion::default();
        let group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Gpu,
            &expansion,
            |_| SmokeMessage,
            [GroupItem::reading(SmokeReading, HoverTarget::Workspace(7))],
        );
        let (entries, favicons, controller_artwork) = (
            DesktopIndex::default(),
            Favicons::default(),
            ControllerArtwork::decoded(),
        );
        let hover_card = HoverCard::empty();
        let surfaces = GroupMenuView::new(
            GroupMenuTopic::Hardware,
            group_menus,
            |_| SmokeMessage,
            [group],
        )
        .surfaces(
            config,
            &ShellLayer::Overlays.default_appearance(),
            &InspectionCard {
                hover_card: &hover_card,
                assets: WindowAssets {
                    entries: &entries,
                    favicons: &favicons,
                    controller_artwork: &controller_artwork,
                },
                clock,
            },
            orientation,
        )
        .expect("an opened menu has a surface");
        let mut primary = Simulator::with_size(
            Settings::default(),
            Size::new(1200.0, 800.0),
            surfaces.primary,
        );
        let header_top = primary
            .find(WidgetGroupFamily::Gpu.title())
            .expect("the group row is on screen")
            .visible_bounds()
            .expect("the group row is visible")
            .y;
        let item_top = primary
            .find("Smoke item")
            .ok()
            .and_then(|found| found.visible_bounds())
            .map(|bounds| bounds.y);
        (header_top, item_top)
    }

    #[test]
    fn every_presentation_lays_out_in_every_direction() {
        let config = Config::default();
        let clock = Clock::real();
        for presentation in every_presentation() {
            let group_menus = opened_menu_state(presentation);
            for direction in [CascadeDirection::Rightward, CascadeDirection::Leftward] {
                for growth in [PanelGrowth::Downward, PanelGrowth::Upward] {
                    let orientation = CascadeOrientation { direction, growth };
                    let (header_top, item_top) =
                        row_tops(&config, &group_menus, orientation, clock);
                    match (presentation.layout, growth) {
                        (GroupMenuLayout::Cascading, _) => assert_eq!(item_top, None),
                        (GroupMenuLayout::Accordion, PanelGrowth::Downward) => {
                            assert!(item_top.is_some_and(|item| item > header_top));
                        }
                        (GroupMenuLayout::Accordion, PanelGrowth::Upward) => {
                            assert!(item_top.is_some_and(|item| item < header_top));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_pinned_composed_item_draws_its_value_on_the_panel() {
        let config = Config::default();
        let clock = Clock::real();
        let target = HoverTarget::Workspace(7);
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::TogglePin {
            menu: GroupMenuTopic::Media,
            entry: GroupMenuPin::Item {
                group: WidgetGroupIdentity::sole(WidgetGroupFamily::Media),
                item_key: target.stable_item_key(),
            },
        });
        let expansion = WidgetGroupExpansion::default();
        let group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Media,
            &expansion,
            |_| SmokeMessage,
            [GroupItem::composed_pinnable(
                ComposedGroupItemFace {
                    name: "Lyrics".into(),
                    icon: GroupItemIcon::Absent,
                },
                target,
                GroupItemValue::drawn(|_| text("the sung line").into()),
            )],
        );
        let panel_views: Vec<_> = GroupMenuView::new(
            GroupMenuTopic::Media,
            &group_menus,
            |_| SmokeMessage,
            [group],
        )
        .panel_views(&config, Position::Top, clock)
        .collect();
        assert_eq!(panel_views.len(), 2);
        let mut panel = Simulator::with_size(
            Settings::default(),
            Size::new(1200.0, 100.0),
            Row::with_children(panel_views),
        );
        assert!(panel.find("the sung line").is_ok());
    }
}
