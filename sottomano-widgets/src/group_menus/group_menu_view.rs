use std::iter::once;

use chrono::Locale;
use corredo::{
    orientation::Axis,
    popovers::{CascadeOrientation, NestedMenuLayout, NestedMenuOpenTrigger},
    theming::text_color,
};
use iced::{Element, widget::Column};
use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot::PanelSlot,
    group_menus::{
        accordion_layout::accordion_sections::accordion_sections,
        cascading_layout::{
            cascade_satellite_panels::CascadeSatellitePanels, cascading_columns::cascading_columns,
        },
        group_menu_state::{
            GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic, MenuPinState,
            OpenGroupMenu, PanelRaise,
        },
        menu_rows::{
            group_menu_group_rows::{GroupMenuGroupRow, GroupMenuRowContext, one_row_per_identity},
            quantity_table::{QuantityTableColumn, quantity_table},
        },
    },
    hover_cards::{
        HoverAnnouncement, HoverTarget, MenuHandleIndicator, PopupMessage, PressHints,
        announcing_hoverable, hover_card_body::hover_card_source_delivery::HoverCard,
        hover_card_content::HoverCardContent, press_toggled_hoverable,
    },
    widget_groups::{
        widget_group_content::{
            EdgeIndicator, GroupEvent, GroupMenuRowParts, GroupToastRequests, PanelPromotion,
            WidgetGroupContent,
        },
        widget_group_identity::{WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        configured_scroll::configured_scroll, icon_glyph::IconGlyph,
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
    pub card_appearance: LayerAppearance,
    pub assets: WindowAssets<'a>,
    pub clock: Clock,
}

impl InspectionCard<'_, '_> {
    pub(crate) fn view(&self, config: &Config) -> Option<Element<'static, PopupMessage>> {
        (!self.hover_card.is_empty()
            || self.hover_card.announcement != HoverAnnouncement::default())
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

    pub(crate) fn intend_group_reveal(&self, group_identity: WidgetGroupIdentity) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::IntendRevealMenuGroup {
            menu: self.topic,
            group_identity,
        })
    }

    pub(crate) fn cancel_group_reveal(&self, group_identity: WidgetGroupIdentity) -> GroupEvent<M> {
        self.sent(GroupMenuMessage::CancelRevealIntent { group_identity })
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

    pub(crate) fn item_list_scroll_message(&self) -> impl Fn(f32) -> GroupEvent<M> + use<M> {
        let on_message = self.on_message;
        let menu = self.topic;
        move |scrolled_px| {
            GroupEvent::Group(on_message(GroupMenuMessage::ItemListScrolled {
                menu,
                scrolled_px,
            }))
        }
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

    pub fn panel_raises(
        &self,
        promotion_severity: Severity,
        clock: Clock,
    ) -> impl Iterator<Item = PanelRaise> {
        let menu = self.messenger.topic;
        self.groups
            .iter()
            .flat_map(move |member| member.panel_raises(menu, promotion_severity, clock))
    }

    fn handle_view(
        &self,
        config: &'a Config,
        panel_position: Position,
        handle_indicator: MenuHandleIndicator,
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
        let glyph = IconGlyph::from(self.messenger.topic.glyph())
            .view(config.theme.font.icon_size_px, text_color);
        let target = HoverTarget::GroupMenu(self.messenger.topic);
        let announcement = HoverAnnouncement::from_group_handle(
            WidgetGroupTitle::fixed(self.messenger.topic.title()),
            PressHints::default(),
        );
        let handle = match self.group_menus.presentation().open_trigger {
            NestedMenuOpenTrigger::Hover => {
                announcing_hoverable(glyph, target, announcement, GroupEvent::HoverCard)
            }
            NestedMenuOpenTrigger::Press => {
                press_toggled_hoverable(glyph, target, announcement, GroupEvent::HoverCard)
            }
        };
        let indicator = EdgeIndicator::of_menu_handle(highest_alert_severity, handle_indicator);
        PanelSlot::new(handle, indicator)
            .view(panel_position)
            .map(M::from)
    }

    pub fn panel_views(
        self,
        config: &'a Config,
        panel_position: Position,
        handle_indicator: MenuHandleIndicator,
        clock: Clock,
    ) -> impl Iterator<Item = Element<'a, M>> + 'a
    where
        M: From<GroupEvent<M>>,
    {
        let handle_slot = self.handle_view(config, panel_position, handle_indicator, clock);
        let topic = self.messenger.topic;
        let group_menus = self.group_menus;
        once(handle_slot).chain(self.groups.into_iter().flat_map(move |member| {
            let group_entry = GroupMenuPin::Group(member.group_identity().clone());
            if group_menus.pin_state(topic, group_entry) == MenuPinState::Pinned {
                return member
                    .pinned_from(topic)
                    .view(config, panel_position, clock)
                    .into_iter()
                    .collect();
            }
            if member.whole_group_promotion() == PanelPromotion::RisesToPanel {
                return member
                    .view(config, panel_position, clock)
                    .into_iter()
                    .collect();
            }
            member.panel_item_views(group_menus, topic, config, panel_position, clock)
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
                group_menus.pin_state(messenger.topic, group_entry) == MenuPinState::Unpinned
            })
            .filter_map(|member| {
                member.menu_member(config, card.clock, group_menus, messenger.topic)
            })
            .fold(MenuSections::empty(), MenuSections::with_member);
        let table = quantity_table(table_columns, config).map(|table| table.map(M::from));
        let cascade_satellite_panels = CascadeSatellitePanels::new(
            appearance,
            &card.card_appearance,
            config,
            orientation,
            group_menus.presentation().submenu_connector,
        );
        let table_edge_fade = cascade_satellite_panels.rows_edge_fade(config);
        let row_context = GroupMenuRowContext {
            config,
            direction: orientation.direction,
            presentation: group_menus.presentation(),
            menu: &messenger,
            group_menus,
            cascade_satellite_panels: &cascade_satellite_panels,
        };
        let group_rows = one_row_per_identity(group_rows);
        let scrolled_px = group_menus.scrolled_px(messenger.topic);
        let scroll_message = messenger.scroll_message();
        let cascade = match row_context.presentation.layout {
            NestedMenuLayout::Cascading => cascading_columns(group_rows, &row_context, card),
            NestedMenuLayout::Accordion => accordion_sections(group_rows, &row_context, card),
        };
        let Some(table) = table else {
            return cascade;
        };
        let Some(cascade) = cascade else {
            return Some(GroupMenuCascadeSurfaces {
                primary: configured_scroll(table, Axis::Vertical, config)
                    .edge_fade(table_edge_fade)
                    .controlled(scrolled_px, move |scrolled_px| {
                        M::from(scroll_message(scrolled_px))
                    })
                    .into(),
                satellites: Vec::new(),
            });
        };
        Some(GroupMenuCascadeSurfaces {
            primary: Column::with_children([table, cascade.primary]).into(),
            satellites: cascade.satellites,
        })
    }
}

#[cfg(test)]
mod tests {
    use corredo::popovers::{
        CascadeDirection, CascadeOrientation, NestedMenuLayout, NestedMenuOpenTrigger,
        NestedMenuPresentation, OpenGroupLimit, PopupGrowth, SubmenuConnector,
    };
    use iced::{
        Element, Event, Point, Settings, Size, mouse,
        widget::{Row, text},
    };
    use iced_controller_diagram::ControllerArtwork;
    use iced_test::{Simulator, simulator::click};
    use sottomano_toml_config::{Config, screen_edges::Position};

    use super::{GroupEvent, GroupMenuCascadeSurfaces, GroupMenuView, InspectionCard};
    use crate::{
        clock::Clock,
        desktop_index::DesktopIndex,
        group_items::{
            group_item::{
                ComposedGroupItemFace, GroupItem, GroupItemHoverArea, GroupItemIcon, GroupItemValue,
            },
            group_item_reading::NamedTestReading,
        },
        group_menus::{
            group_menu_state::{GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic},
            menu_rows::menu_row::menu_row_height,
        },
        hover_cards::{
            HoverAnnouncement, HoverTarget, MenuHandleIndicator, PopupMessage, PressHints,
            hover_card_body::hover_card_source_delivery::HoverCard,
        },
        widget_groups::{
            widget_group_content::{WidgetGroupContent, WidgetGroupExpansion},
            widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
        },
        widgets::{
            favicon::Favicons, shell_layer_appearance::ShellLayer, window_icon::WindowAssets,
        },
    };

    struct MenuTestBench {
        config: Config,
        hover_card: HoverCard,
        entries: DesktopIndex,
        favicons: Favicons,
        controller_artwork: ControllerArtwork,
        clock: Clock,
    }

    impl Default for MenuTestBench {
        fn default() -> Self {
            MenuTestBench {
                config: Config::default(),
                hover_card: HoverCard::empty(),
                entries: DesktopIndex::default(),
                favicons: Favicons::default(),
                controller_artwork: ControllerArtwork::decoded(),
                clock: Clock::real(),
            }
        }
    }

    impl MenuTestBench {
        fn inspection_card(&self) -> InspectionCard<'_, '_> {
            InspectionCard {
                hover_card: &self.hover_card,
                card_appearance: ShellLayer::Overlays.default_appearance(),
                assets: WindowAssets {
                    entries: &self.entries,
                    favicons: &self.favicons,
                    controller_artwork: &self.controller_artwork,
                },
                clock: self.clock,
            }
        }
    }

    impl<'a, M: Clone + From<GroupEvent<M>> + From<PopupMessage> + 'a> GroupMenuView<'a, M> {
        fn bench_surfaces(
            self,
            bench: &'a MenuTestBench,
            orientation: CascadeOrientation,
        ) -> GroupMenuCascadeSurfaces<'a, M> {
            self.surfaces(
                &bench.config,
                &ShellLayer::Overlays.default_appearance(),
                &bench.inspection_card(),
                orientation,
            )
            .expect("an opened menu has a surface")
        }
    }

    impl<'a, M> GroupMenuCascadeSurfaces<'a, M> {
        fn first_item_list(self) -> Element<'a, M> {
            self.satellites
                .into_iter()
                .next()
                .expect("the open group has an item list")
        }
    }

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

    fn every_presentation() -> Vec<NestedMenuPresentation> {
        let layouts = [NestedMenuLayout::Cascading, NestedMenuLayout::Accordion];
        let triggers = [NestedMenuOpenTrigger::Hover, NestedMenuOpenTrigger::Press];
        let limits = [OpenGroupLimit::One, OpenGroupLimit::Several];
        let connectors = [SubmenuConnector::Hidden, SubmenuConnector::Shown];
        layouts
            .into_iter()
            .flat_map(|layout| {
                triggers.into_iter().flat_map(move |open_trigger| {
                    limits.into_iter().flat_map(move |accordion_open_limit| {
                        connectors.into_iter().map(move |submenu_connector| {
                            NestedMenuPresentation {
                                layout,
                                open_trigger,
                                accordion_open_limit,
                                submenu_connector,
                            }
                        })
                    })
                })
            })
            .collect()
    }

    fn opened_menu_state(presentation: NestedMenuPresentation) -> GroupMenuState {
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(presentation));
        let _ = group_menus.update(GroupMenuMessage::RevealMenuGroup {
            menu: GroupMenuTopic::Hardware,
            group_identity: WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu),
        });
        group_menus
    }

    fn row_tops(
        group_menus: &GroupMenuState,
        orientation: CascadeOrientation,
        clock: Clock,
    ) -> (f32, Option<f32>) {
        let bench = MenuTestBench {
            clock,
            ..MenuTestBench::default()
        };
        let expansion = WidgetGroupExpansion::default();
        let group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Gpu,
            &expansion,
            |_| SmokeMessage,
            [GroupItem::reading(
                NamedTestReading("Smoke item"),
                HoverTarget::Workspace(7),
            )],
        );
        let surfaces = GroupMenuView::new(
            GroupMenuTopic::Hardware,
            group_menus,
            |_| SmokeMessage,
            [group],
        )
        .bench_surfaces(&bench, orientation);
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
        let clock = Clock::real();
        for presentation in every_presentation() {
            let group_menus = opened_menu_state(presentation);
            for direction in [CascadeDirection::Rightward, CascadeDirection::Leftward] {
                for growth in [PopupGrowth::Downward, PopupGrowth::Upward] {
                    let orientation = CascadeOrientation { direction, growth };
                    let (header_top, item_top) = row_tops(&group_menus, orientation, clock);
                    match presentation.layout {
                        NestedMenuLayout::Cascading => assert_eq!(item_top, None),
                        NestedMenuLayout::Accordion => {
                            assert!(item_top.is_some_and(|item| item > header_top));
                        }
                    }
                }
            }
        }
    }

    fn long_item_list_row_top(
        screen_height: f32,
        item_list_scrolled_px: f32,
        row_name: &str,
    ) -> f32 {
        let expansion = WidgetGroupExpansion::default();
        let long_list = (0..40).map(|position| {
            GroupItem::composed(
                ComposedGroupItemFace {
                    name: format!("Row {position}").into(),
                    icon: GroupItemIcon::Absent,
                },
                GroupItemHoverArea::WholeItem(HoverTarget::Workspace(position)),
                GroupItemValue::Nothing,
            )
        });
        let group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Media,
            &expansion,
            |_| SmokeMessage,
            long_list,
        );
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(
            NestedMenuPresentation {
                layout: NestedMenuLayout::Cascading,
                ..NestedMenuPresentation::default()
            },
        ));
        let _ = group_menus.update(GroupMenuMessage::RevealMenuGroup {
            menu: GroupMenuTopic::Media,
            group_identity: WidgetGroupIdentity::sole(WidgetGroupFamily::Media),
        });
        let _ = group_menus.update(GroupMenuMessage::ItemListScrolled {
            menu: GroupMenuTopic::Media,
            scrolled_px: item_list_scrolled_px,
        });
        let bench = MenuTestBench::default();
        let surfaces = GroupMenuView::new(
            GroupMenuTopic::Media,
            &group_menus,
            |_| SmokeMessage,
            [group],
        )
        .bench_surfaces(
            &bench,
            CascadeOrientation {
                direction: CascadeDirection::Rightward,
                growth: PopupGrowth::Downward,
            },
        );
        let item_list = surfaces.first_item_list();
        Simulator::with_size(
            Settings::default(),
            Size::new(600.0, screen_height),
            item_list,
        )
        .find(row_name)
        .expect("the row is laid out")
        .bounds()
        .y
    }

    #[test]
    fn an_item_list_that_fits_the_screen_never_scrolls() {
        let row_pitch = menu_row_height(&Config::default());
        let screen_height = row_pitch * 40.0;
        let unscrolled = long_item_list_row_top(screen_height, 0.0, "Row 0");
        let scrolled_past_the_end = long_item_list_row_top(screen_height, f32::MAX, "Row 0");
        assert!((scrolled_past_the_end - unscrolled).abs() < 1.0);
    }

    #[test]
    fn an_item_list_taller_than_the_screen_scrolls_to_its_last_rows() {
        let row_pitch = menu_row_height(&Config::default());
        let screen_height = row_pitch * 10.0;
        let first_row_top = long_item_list_row_top(screen_height, 0.0, "Row 0");
        let first_row_of_the_last_page = long_item_list_row_top(screen_height, f32::MAX, "Row 30");
        assert!((first_row_of_the_last_page - first_row_top).abs() < 1.0);
    }

    fn inspection_card_view(hover_card: HoverCard) -> Option<Element<'static, PopupMessage>> {
        let bench = MenuTestBench {
            hover_card,
            ..MenuTestBench::default()
        };
        bench.inspection_card().view(&bench.config)
    }

    #[test]
    fn an_announcement_only_card_is_shown_in_a_menu() {
        let announced =
            HoverCard::empty().with_announcement(HoverAnnouncement::from_reading(PressHints {
                left: Some("switch something"),
                ..PressHints::default()
            }));
        assert!(inspection_card_view(announced).is_some());
    }

    #[test]
    fn a_card_without_sections_or_announcement_stays_hidden() {
        assert!(inspection_card_view(HoverCard::empty()).is_none());
    }

    #[derive(Debug, Clone, PartialEq)]
    enum RowPointerMessage {
        RowInspected(HoverTarget),
        RowAction,
        Revealed(WidgetGroupIdentity),
        RevealIntended(WidgetGroupIdentity),
        RevealCancelled(WidgetGroupIdentity),
        Unrelated,
    }

    impl From<GroupEvent<RowPointerMessage>> for RowPointerMessage {
        fn from(event: GroupEvent<RowPointerMessage>) -> Self {
            match event {
                GroupEvent::Group(message) => message,
                GroupEvent::HoverCard(_) => RowPointerMessage::Unrelated,
            }
        }
    }

    impl From<PopupMessage> for RowPointerMessage {
        fn from(_: PopupMessage) -> Self {
            RowPointerMessage::Unrelated
        }
    }

    const POINTED_ROW: HoverTarget = HoverTarget::Workspace(7);

    #[derive(Clone, Copy)]
    enum PointedRowPress {
        PerformsAction,
        ReadOnly,
    }

    fn messages_after_pointing(
        pointer_target: impl Fn(&mut Simulator<'_, RowPointerMessage>) -> Point,
        pressed: bool,
    ) -> Vec<RowPointerMessage> {
        messages_after_pointing_at_row(PointedRowPress::PerformsAction, pointer_target, pressed)
    }

    fn messages_after_pointing_at_row(
        row_press: PointedRowPress,
        pointer_target: impl Fn(&mut Simulator<'_, RowPointerMessage>) -> Point,
        pressed: bool,
    ) -> Vec<RowPointerMessage> {
        let bench = MenuTestBench::default();
        let expansion = WidgetGroupExpansion::default();
        let group_menus = opened_menu_state(NestedMenuPresentation::default());
        let group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Gpu,
            &expansion,
            |_| RowPointerMessage::Unrelated,
            [match row_press {
                PointedRowPress::PerformsAction => {
                    GroupItem::reading(NamedTestReading("Smoke item"), POINTED_ROW)
                        .on_press("act", RowPointerMessage::RowAction)
                }
                PointedRowPress::ReadOnly => {
                    GroupItem::reading(NamedTestReading("Smoke item"), POINTED_ROW)
                }
            }],
        );
        let surfaces = GroupMenuView::new(
            GroupMenuTopic::Hardware,
            &group_menus,
            |message| match message {
                GroupMenuMessage::InspectRow { target, .. } => {
                    RowPointerMessage::RowInspected(target)
                }
                _ => RowPointerMessage::Unrelated,
            },
            [group],
        )
        .bench_surfaces(
            &bench,
            CascadeOrientation {
                direction: CascadeDirection::Rightward,
                growth: PopupGrowth::Downward,
            },
        );
        let item_list = surfaces.first_item_list();
        let mut simulator =
            Simulator::with_size(Settings::default(), Size::new(1200.0, 800.0), item_list);
        let pointer = pointer_target(&mut simulator);
        simulator.point_at(pointer);
        let moved_events = [Event::Mouse(mouse::Event::CursorMoved {
            position: pointer,
        })];
        let _statuses = simulator.simulate(moved_events);
        if pressed {
            let _statuses = simulator.simulate(click());
        }
        simulator.into_messages().collect()
    }

    fn row_name_center(simulator: &mut Simulator<'_, RowPointerMessage>) -> Point {
        simulator
            .find("Smoke item")
            .expect("the row is laid out")
            .visible_bounds()
            .expect("the row name is visible")
            .center()
    }

    #[derive(Clone, Copy)]
    enum PointerApproach {
        StraightDown,
        TowardTheSatellite,
    }

    fn compute_reveal_messages(approach: PointerApproach) -> Vec<RowPointerMessage> {
        let bench = MenuTestBench::default();
        let expansion = WidgetGroupExpansion::default();
        let group_menus = opened_menu_state(NestedMenuPresentation::default());
        let groups = [WidgetGroupFamily::Gpu, WidgetGroupFamily::Compute].map(|family| {
            WidgetGroupContent::expandable(
                family,
                &expansion,
                |_| RowPointerMessage::Unrelated,
                [GroupItem::reading(
                    NamedTestReading("Smoke item"),
                    POINTED_ROW,
                )],
            )
        });
        let surfaces = GroupMenuView::new(
            GroupMenuTopic::Hardware,
            &group_menus,
            |message| match message {
                GroupMenuMessage::RevealMenuGroup { group_identity, .. } => {
                    RowPointerMessage::Revealed(group_identity)
                }
                GroupMenuMessage::IntendRevealMenuGroup { group_identity, .. } => {
                    RowPointerMessage::RevealIntended(group_identity)
                }
                GroupMenuMessage::CancelRevealIntent { group_identity } => {
                    RowPointerMessage::RevealCancelled(group_identity)
                }
                _ => RowPointerMessage::Unrelated,
            },
            groups,
        )
        .bench_surfaces(
            &bench,
            CascadeOrientation {
                direction: CascadeDirection::Rightward,
                growth: PopupGrowth::Downward,
            },
        );
        let mut simulator = Simulator::with_size(
            Settings::default(),
            Size::new(1200.0, 800.0),
            surfaces.primary,
        );
        let row_center = simulator
            .find("Compute")
            .expect("the group row is laid out")
            .visible_bounds()
            .expect("the group row is visible")
            .center();
        let approach_start = match approach {
            PointerApproach::StraightDown => Point::new(row_center.x, 0.0),
            PointerApproach::TowardTheSatellite => {
                Point::new((row_center.x - row_center.y).max(0.0), 0.0)
            }
        };
        for pointer in [approach_start, row_center, Point::new(1190.0, 790.0)] {
            simulator.point_at(pointer);
            let _statuses = simulator.simulate([Event::Mouse(mouse::Event::CursorMoved {
                position: pointer,
            })]);
        }
        let compute = WidgetGroupIdentity::sole(WidgetGroupFamily::Compute);
        simulator
            .into_messages()
            .filter(|message| match message {
                RowPointerMessage::Revealed(identity)
                | RowPointerMessage::RevealIntended(identity)
                | RowPointerMessage::RevealCancelled(identity) => *identity == compute,
                _ => false,
            })
            .collect()
    }

    #[test]
    fn pointing_straight_down_onto_a_group_row_reveals_it_at_once() {
        let compute = WidgetGroupIdentity::sole(WidgetGroupFamily::Compute);
        assert_eq!(
            compute_reveal_messages(PointerApproach::StraightDown),
            [
                RowPointerMessage::Revealed(compute.clone()),
                RowPointerMessage::RevealCancelled(compute)
            ]
        );
    }

    #[test]
    fn crossing_a_group_row_toward_the_open_satellite_defers_its_reveal() {
        let compute = WidgetGroupIdentity::sole(WidgetGroupFamily::Compute);
        assert_eq!(
            compute_reveal_messages(PointerApproach::TowardTheSatellite),
            [
                RowPointerMessage::RevealIntended(compute.clone()),
                RowPointerMessage::RevealCancelled(compute)
            ]
        );
    }

    fn row_icon_center(simulator: &mut Simulator<'_, RowPointerMessage>) -> Point {
        let row_name_bounds = simulator
            .find("Smoke item")
            .expect("the row is laid out")
            .visible_bounds()
            .expect("the row name is visible");
        Point::new(row_name_bounds.x - 24.0, row_name_bounds.center_y())
    }

    #[test]
    fn pressing_the_row_icon_performs_the_row_action() {
        assert!(
            messages_after_pointing(row_icon_center, true).contains(&RowPointerMessage::RowAction)
        );
    }

    #[test]
    fn hovering_the_row_name_inspects_the_row() {
        assert!(
            messages_after_pointing(row_name_center, false)
                .contains(&RowPointerMessage::RowInspected(POINTED_ROW))
        );
    }

    #[test]
    fn pressing_the_row_name_performs_the_row_action() {
        assert!(
            messages_after_pointing(row_name_center, true).contains(&RowPointerMessage::RowAction)
        );
    }

    #[test]
    fn pressing_a_read_only_row_name_sends_no_row_message() {
        let messages =
            messages_after_pointing_at_row(PointedRowPress::ReadOnly, row_name_center, true);
        assert!(!messages.contains(&RowPointerMessage::RowAction));
    }

    #[test]
    fn hovering_a_read_only_row_inspects_it() {
        assert!(
            messages_after_pointing_at_row(PointedRowPress::ReadOnly, row_name_center, false)
                .contains(&RowPointerMessage::RowInspected(POINTED_ROW))
        );
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
        .panel_views(
            &config,
            Position::Top,
            MenuHandleIndicator::default(),
            clock,
        )
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
