use std::iter::once;

use chrono::Locale;
use corredo::{
    orientation::Axis,
    popovers::{
        BesideRow, CascadeOrientation, NestedMenuGroup, NestedMenuLayout, NestedMenuRowEmphasis,
        NestedMenuStyle, nested_menu,
    },
    selection_scroll::{ScrollEdgeFade, ScrollPacing},
    theming::text_color,
    transitions::ExpansionState,
};
use iced::{Element, widget::Column};
use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use crate::{
    clock::Clock,
    desktop_panels::panel_slot::PanelSlot,
    group_indicators::group_indicator::{GroupIndicator, RowHoverCard},
    group_menus::{
        cascading_layout::menu_satellite_regions::{SatelliteBlur, satellite_with_regions},
        group_menu_state::{
            AlertPromotion, GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
            OpenGroupMenu, PanelRaise,
        },
        menu_rows::{
            group_menu_group_rows::{
                GroupMenuGroupRow, GroupMenuRowContext, group_header, indicator_entry,
                one_row_per_identity,
            },
            menu_row::{menu_metrics, menu_row_height},
            quantity_table::{QuantityTableColumn, quantity_table},
        },
    },
    hover_cards::{
        CardHeader, HoverTarget, IndicatorKey, MenuHandleIndicator, PanelPinOrigin, PopupMessage,
        hover_card_body::{Section, hover_card_source_delivery::HoverCard},
        hover_card_content::HoverCardContent,
        hoverable,
    },
    widget_groups::{
        widget_group_content::{
            GroupEvent, GroupToastRequests, WidgetGroupContent, edge_indicator,
        },
        widget_group_identity::WidgetGroupIdentity,
        widget_group_interaction::WidgetGroupInteraction,
    },
    widgets::{
        configured_scroll::configured_scroll, icon_glyph::IconGlyph,
        shell_layer_appearance::LayerAppearance, window_icon::WindowAssets,
    },
};

pub struct GroupMenuView<'a> {
    group_menus: &'a GroupMenuState,
    messenger: GroupMenuMessenger,
    groups: Vec<WidgetGroupContent<'a>>,
}

pub struct GroupMenuCascadeSurfaces<'a, Message> {
    pub primary: Element<'a, Message>,
    pub satellites: Vec<Element<'a, Message>>,
}

pub struct InspectionCard<'c, 'a> {
    pub sections: &'c [Section],
    pub card_appearance: LayerAppearance,
    pub assets: WindowAssets<'a>,
    pub clock: Clock,
}

impl InspectionCard<'_, '_> {
    pub(crate) fn view(&self, config: &Config) -> Option<Element<'static, PopupMessage>> {
        let hover_card = HoverCard::new(self.sections.to_vec(), CardHeader::Unheaded)?;
        Some(HoverCardContent::new(&hover_card, config, self.assets, self.clock).view())
    }
}

pub(crate) struct GroupMenuMessenger {
    pub(crate) topic: GroupMenuTopic,
}

pub(crate) fn menu_event(message: GroupMenuMessage) -> GroupEvent {
    GroupEvent::Interaction(WidgetGroupInteraction::GroupMenus(message))
}

impl GroupMenuMessenger {
    pub(crate) fn toggle_pin(&self, entry: GroupMenuPin) -> GroupEvent {
        menu_event(GroupMenuMessage::TogglePin {
            menu: self.topic,
            entry,
        })
    }

    pub(crate) fn reveal_group(&self, group_identity: WidgetGroupIdentity) -> GroupEvent {
        menu_event(GroupMenuMessage::RevealMenuGroup {
            menu: self.topic,
            group_identity,
        })
    }

    pub(crate) fn intend_group_reveal(&self, group_identity: WidgetGroupIdentity) -> GroupEvent {
        menu_event(GroupMenuMessage::IntendRevealMenuGroup {
            menu: self.topic,
            group_identity,
        })
    }

    pub(crate) fn toggle_group(&self, group_identity: WidgetGroupIdentity) -> GroupEvent {
        menu_event(GroupMenuMessage::ToggleMenuGroup {
            menu: self.topic,
            group_identity,
        })
    }

    pub(crate) fn scroll_message(&self) -> impl Fn(f32) -> GroupEvent + use<> {
        let menu = self.topic;
        move |scrolled_px| menu_event(GroupMenuMessage::MenuScrolled { menu, scrolled_px })
    }

    pub(crate) fn indicator_list_scroll_message(&self) -> impl Fn(f32) -> GroupEvent + use<> {
        let menu = self.topic;
        move |scrolled_px| menu_event(GroupMenuMessage::ItemListScrolled { menu, scrolled_px })
    }

    pub(crate) fn inspect_row(&self, indicator: IndicatorKey) -> GroupEvent {
        menu_event(GroupMenuMessage::InspectRow {
            menu: self.topic,
            indicator,
        })
    }
}

pub(crate) struct MenuSections<'a> {
    pub table_columns: Vec<QuantityTableColumn<'a>>,
    pub group_rows: Vec<GroupMenuGroupRow<'a>>,
}

impl MenuSections<'_> {
    fn empty() -> Self {
        Self {
            table_columns: Vec::new(),
            group_rows: Vec::new(),
        }
    }
}

impl<'a> GroupMenuView<'a> {
    pub fn new(
        topic: GroupMenuTopic,
        group_menus: &'a GroupMenuState,
        groups: impl IntoIterator<Item = WidgetGroupContent<'a>>,
    ) -> Self {
        Self {
            group_menus,
            messenger: GroupMenuMessenger { topic },
            groups: groups.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn indicator(&self, indicator_key: &IndicatorKey) -> Option<&GroupIndicator<'a>> {
        self.groups
            .iter()
            .find_map(|group| group.indicator(indicator_key))
    }

    #[must_use]
    pub fn hovered_indicator(&self, target: &HoverTarget) -> Option<&GroupIndicator<'a>> {
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
                .map(|group| group.group_identity().family)
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
        config: &'a Config,
        panel_position: Position,
        handle_indicator: MenuHandleIndicator,
    ) -> Element<'a, GroupEvent> {
        let highest_alert_severity = self
            .groups
            .iter()
            .map(WidgetGroupContent::highest_alert_severity)
            .max()
            .unwrap_or_default();
        let glyph = IconGlyph::from(self.messenger.topic.glyph())
            .view(config.theme.font.icon_size_px, text_color);
        let handle = hoverable(
            self.group_menus.presentation().open_trigger,
            glyph,
            HoverTarget::GroupMenu(self.messenger.topic),
            GroupEvent::from,
        );
        let edge_indicator_view = edge_indicator(
            highest_alert_severity,
            handle_indicator,
            handle_indicator.settled_reveal(),
        );
        PanelSlot::new(handle, edge_indicator_view).view(panel_position)
    }

    pub fn panel_views(
        self,
        config: &'a Config,
        panel_position: Position,
        handle_indicator: MenuHandleIndicator,
        clock: Clock,
    ) -> impl Iterator<Item = Element<'a, GroupEvent>> + 'a {
        let handle_slot = self.handle_view(config, panel_position, handle_indicator);
        let topic = self.messenger.topic;
        let group_menus = self.group_menus;
        once(handle_slot).chain(self.groups.into_iter().flat_map(move |member| {
            let group_entry = GroupMenuPin::Group(member.group_identity().clone());
            if let PanelPinOrigin::Pinned(_) =
                group_menus.panel_pin(topic, group_entry, AlertPromotion::Unpromoted)
            {
                return member
                    .pinned_from(topic)
                    .view(config, panel_position, clock)
                    .into_iter()
                    .collect();
            }
            member.panel_indicator_views(group_menus, topic, config, panel_position, clock)
        }))
    }

    #[must_use]
    pub fn surfaces(
        self,
        config: &'a Config,
        appearance: &LayerAppearance,
        card: &InspectionCard<'_, 'a>,
        orientation: CascadeOrientation,
    ) -> Option<GroupMenuCascadeSurfaces<'a, GroupEvent>> {
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
                group_menus.panel_pin(messenger.topic, group_entry, AlertPromotion::Unpromoted)
                    == PanelPinOrigin::Unpinned
            })
            .fold(MenuSections::empty(), |sections, member| {
                member.extend_menu_sections(
                    config,
                    card.clock,
                    group_menus,
                    messenger.topic,
                    sections,
                )
            });
        let table = quantity_table(table_columns, config);
        let topic = messenger.topic;
        let presentation = group_menus.presentation();
        let row_context = GroupMenuRowContext {
            config,
            direction: orientation.direction,
            presentation,
            menu: &messenger,
            group_menus,
            clock: card.clock,
        };
        let menu_groups = nested_menu_groups(&row_context, group_rows, card);
        let list_frame = appearance.frame;
        let fade_extent_px = config.group_menus.scroll_fade_rows * menu_row_height(config);
        let scrolled_px = group_menus.scrolled_px(topic);
        let list_scroll_message = messenger.scroll_message();
        let indicator_list_scroll_message = messenger.indicator_list_scroll_message();
        let satellite_blur = SatelliteBlur {
            item_list: appearance.backdrop_blur,
            beside: card.card_appearance.backdrop_blur,
        };
        let cascade = nested_menu(
            menu_groups,
            presentation.layout,
            orientation,
            menu_metrics(config),
            list_scroll_message,
            indicator_list_scroll_message,
        )
        .style(NestedMenuStyle {
            list_frame,
            beside_frame: card.card_appearance.frame,
            morph: config.overlay_surfaces.surface_morph_duration,
            gap_to_parent_px: config.theme.spacing.item_gap,
            submenu_connector: presentation.submenu_connector,
            fade_extent_px,
            item_indent_px: config.theme.font.icon_size_px,
        })
        .wheel_scrolling(
            config.scrolling.wheel_line_px,
            ScrollPacing::Eased(config.scrolling.wheel_easing),
        )
        .scrolled(scrolled_px, group_menus.item_list_scrolled_px(topic))
        .surfaces()
        .map(|surfaces| GroupMenuCascadeSurfaces {
            primary: surfaces.primary,
            satellites: surfaces
                .satellites
                .into_iter()
                .map(|satellite| satellite_with_regions(satellite, satellite_blur))
                .collect(),
        });
        let Some(table) = table else {
            return cascade;
        };
        let Some(cascade) = cascade else {
            let table_scroll_message = messenger.scroll_message();
            return Some(GroupMenuCascadeSurfaces {
                primary: configured_scroll(table, Axis::Vertical, config)
                    .edge_fade(ScrollEdgeFade::within_frame(list_frame, fade_extent_px))
                    .controlled(scrolled_px, move |scrolled_px| {
                        table_scroll_message(scrolled_px)
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

fn nested_menu_groups<'a>(
    row_context: &GroupMenuRowContext<'_>,
    group_rows: Vec<GroupMenuGroupRow<'a>>,
    card: &InspectionCard<'_, 'a>,
) -> Vec<NestedMenuGroup<'a, IndicatorKey, GroupEvent>> {
    let topic = row_context.menu.topic;
    let open_groups = row_context.group_menus.open_groups(topic);
    let inspected_indicator = row_context.group_menus.inspected_indicator(topic);
    one_row_per_identity(group_rows)
        .into_iter()
        .map(|entry| {
            let expansion_state = if open_groups.contains(&entry.group_identity) {
                ExpansionState::Expanded
            } else {
                ExpansionState::Collapsed
            };
            let emphasis = match (row_context.presentation.layout, expansion_state) {
                (NestedMenuLayout::Cascading, ExpansionState::Expanded) => {
                    NestedMenuRowEmphasis::Selected
                }
                (NestedMenuLayout::Accordion, ExpansionState::Expanded) => {
                    NestedMenuRowEmphasis::Opened
                }
                (_, ExpansionState::Collapsed) => NestedMenuRowEmphasis::Plain,
            };
            let header = group_header(
                row_context,
                &entry.group_identity,
                &entry.group_title,
                &entry.headline_indicator,
                emphasis,
            );
            let indicators = entry
                .indicators
                .iter()
                .map(|indicator| {
                    let beside =
                        match indicator.row_emphasis(&entry.group_identity, inspected_indicator) {
                            NestedMenuRowEmphasis::Selected => card
                                .view(row_context.config)
                                .map_or(BesideRow::Nothing, |card_view| {
                                    BesideRow::Card(card_view.map(GroupEvent::Popup))
                                }),
                            NestedMenuRowEmphasis::Plain | NestedMenuRowEmphasis::Opened => {
                                BesideRow::Nothing
                            }
                        };
                    indicator_entry(row_context, &entry.group_identity, indicator, beside)
                })
                .collect();
            NestedMenuGroup {
                header,
                expansion_state,
                items: indicators,
            }
        })
        .collect()
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
        group_indicators::{
            group_indicator::{GroupIndicator, IndicatorValue, named_test_indicator},
            indicator_drawing::IndicatorIcon,
            indicator_state::IndicatorState,
        },
        group_menus::{
            group_menu_state::{GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic},
            menu_rows::menu_row::menu_row_height,
        },
        hover_cards::{
            IndicatorKey, MenuHandleIndicator, MouseGesture, PopupMessage, PressAction,
            hover_card_body::Section,
        },
        widget_groups::{
            clock_group::ClockInteraction,
            compositor_styling_group::CompositorStylingMessage,
            widget_group_content::{WidgetGroupContent, WidgetGroupExpansion},
            widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity},
            widget_group_interaction::WidgetGroupInteraction,
        },
        widgets::{
            favicon::Favicons, icon_glyph::IconGlyph, shell_layer_appearance::ShellLayer,
            window_icon::WindowAssets,
        },
    };

    struct MenuTestBench {
        config: Config,
        sections: Vec<Section>,
        entries: DesktopIndex,
        favicons: Favicons,
        controller_artwork: ControllerArtwork,
        clock: Clock,
    }

    impl Default for MenuTestBench {
        fn default() -> Self {
            MenuTestBench {
                config: Config::default(),
                sections: Vec::new(),
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
                sections: &self.sections,
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

    impl<'a> GroupMenuView<'a> {
        fn bench_surfaces(
            self,
            bench: &'a MenuTestBench,
            orientation: CascadeOrientation,
        ) -> GroupMenuCascadeSurfaces<'a, GroupEvent> {
            self.surfaces(
                &bench.config,
                &ShellLayer::Overlays.default_appearance(),
                &bench.inspection_card(),
                orientation,
            )
            .expect("an opened menu has a surface")
        }
    }

    impl<'a> GroupMenuCascadeSurfaces<'a, GroupEvent> {
        fn first_indicator_list(self) -> Element<'a, GroupEvent> {
            self.satellites
                .into_iter()
                .next()
                .expect("the open group has an item list")
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
            GroupIndicator::glyph_only(
                WidgetGroupFamily::Gpu.title(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            &expansion,
            ClockInteraction::Expansion,
            [named_test_indicator("Smoke item")],
        );
        let surfaces = GroupMenuView::new(GroupMenuTopic::Hardware, group_menus, [group])
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
        let indicator_top = primary
            .find("Smoke item")
            .ok()
            .and_then(|found| found.visible_bounds())
            .map(|bounds| bounds.y);
        (header_top, indicator_top)
    }

    #[test]
    fn a_group_without_indicators_leaves_the_menu() {
        let group_menus = opened_menu_state(every_presentation()[0]);
        let expansion = WidgetGroupExpansion::default();
        let empty_group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Gpu,
            GroupIndicator::glyph_only(
                WidgetGroupFamily::Gpu.title(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            &expansion,
            ClockInteraction::Expansion,
            Vec::<GroupIndicator<'_>>::new(),
        );
        let menu = GroupMenuView::new(GroupMenuTopic::Hardware, &group_menus, [empty_group]);
        assert_eq!(menu.open_group_menu().families.len(), 1);
    }

    #[test]
    fn a_headline_indicator_is_not_listed_among_the_group_indicators() {
        let group_menus = opened_menu_state(every_presentation()[0]);
        let expansion = WidgetGroupExpansion::default();
        let headline_group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Gpu,
            named_test_indicator("Headline").with_headline_glyph(IconGlyph::default()),
            &expansion,
            ClockInteraction::Expansion,
            [named_test_indicator("Listed")],
        );
        let indicator_key = |name: &str| IndicatorKey {
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu),
            name: name.to_owned().into(),
        };
        let menu = GroupMenuView::new(GroupMenuTopic::Hardware, &group_menus, [headline_group]);
        assert!(menu.indicator(&indicator_key("Headline")).is_none());
        assert!(menu.indicator(&indicator_key("Listed")).is_some());
    }

    #[test]
    fn every_presentation_lays_out_in_every_direction() {
        let clock = Clock::real();
        for presentation in every_presentation() {
            let group_menus = opened_menu_state(presentation);
            for direction in [CascadeDirection::Rightward, CascadeDirection::Leftward] {
                for growth in [PopupGrowth::Downward, PopupGrowth::Upward] {
                    let orientation = CascadeOrientation { direction, growth };
                    let (header_top, indicator_top) = row_tops(&group_menus, orientation, clock);
                    match presentation.layout {
                        NestedMenuLayout::Cascading => assert_eq!(indicator_top, None),
                        NestedMenuLayout::Accordion => {
                            assert!(indicator_top.is_some_and(|indicator| indicator > header_top));
                        }
                    }
                }
            }
        }
    }

    fn long_indicator_list_row_top(
        screen_height: f32,
        item_list_scrolled_px: f32,
        row_name: &str,
    ) -> f32 {
        let expansion = WidgetGroupExpansion::default();
        let long_list = (0..40).map(|position| {
            GroupIndicator::composed(
                format!("Row {position}"),
                IndicatorIcon::Absent,
                IndicatorValue::Nothing,
                Box::new(|_| Vec::new()),
            )
        });
        let group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Media,
            GroupIndicator::glyph_only(
                WidgetGroupFamily::Media.title(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            &expansion,
            ClockInteraction::Expansion,
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
        let surfaces = GroupMenuView::new(GroupMenuTopic::Media, &group_menus, [group])
            .bench_surfaces(
                &bench,
                CascadeOrientation {
                    direction: CascadeDirection::Rightward,
                    growth: PopupGrowth::Downward,
                },
            );
        let item_list = surfaces.first_indicator_list();
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
    fn an_indicator_list_that_fits_the_screen_never_scrolls() {
        let row_pitch = menu_row_height(&Config::default());
        let screen_height = row_pitch * 40.0;
        let unscrolled = long_indicator_list_row_top(screen_height, 0.0, "Row 0");
        let scrolled_past_the_end = long_indicator_list_row_top(screen_height, f32::MAX, "Row 0");
        assert!((scrolled_past_the_end - unscrolled).abs() < 1.0);
    }

    #[test]
    fn an_indicator_list_taller_than_the_screen_scrolls_to_its_last_rows() {
        let row_pitch = menu_row_height(&Config::default());
        let screen_height = row_pitch * 10.0;
        let first_row_top = long_indicator_list_row_top(screen_height, 0.0, "Row 0");
        let first_row_of_the_last_page =
            long_indicator_list_row_top(screen_height, f32::MAX, "Row 30");
        assert!((first_row_of_the_last_page - first_row_top).abs() < 1.0);
    }

    fn inspection_card_view(sections: Vec<Section>) -> Option<Element<'static, PopupMessage>> {
        let bench = MenuTestBench {
            sections,
            ..MenuTestBench::default()
        };
        bench.inspection_card().view(&bench.config)
    }

    #[test]
    fn an_actions_only_card_is_shown_in_a_menu() {
        let actions_only = Section::press_actions(vec![PressAction::new(
            MouseGesture::LeftClick,
            "switch something",
        )]);
        assert!(inspection_card_view(actions_only.into_iter().collect()).is_some());
    }

    #[test]
    fn a_card_without_sections_stays_hidden() {
        assert!(inspection_card_view(Vec::new()).is_none());
    }

    #[derive(Debug, Clone, PartialEq)]
    enum RowPointerMessage {
        RowInspected(IndicatorKey),
        RowAction,
        Revealed(WidgetGroupIdentity),
        RevealIntended(WidgetGroupIdentity),
        RevealCancelled(WidgetGroupIdentity),
        Unrelated,
    }

    impl From<GroupEvent> for RowPointerMessage {
        fn from(event: GroupEvent) -> Self {
            match event {
                GroupEvent::Interaction(WidgetGroupInteraction::CompositorStyling(
                    CompositorStylingMessage::ToggleXray,
                )) => RowPointerMessage::RowAction,
                GroupEvent::Interaction(WidgetGroupInteraction::GroupMenus(menu_message)) => {
                    match menu_message {
                        GroupMenuMessage::InspectRow { indicator, .. } => {
                            RowPointerMessage::RowInspected(indicator)
                        }
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
                    }
                }
                GroupEvent::Interaction(_) | GroupEvent::Popup(_) => RowPointerMessage::Unrelated,
            }
        }
    }

    fn pointed_row() -> IndicatorKey {
        IndicatorKey {
            group: WidgetGroupIdentity::sole(WidgetGroupFamily::Gpu),
            name: "Smoke item".into(),
        }
    }

    #[derive(Clone, Copy)]
    enum PointedRowPress {
        PerformsAction,
        ReadOnly,
    }

    fn messages_after_pointing(
        pointer_target: impl Fn(&mut Simulator<'_, GroupEvent>) -> Point,
        pressed: bool,
    ) -> Vec<RowPointerMessage> {
        messages_after_pointing_at_row(PointedRowPress::PerformsAction, pointer_target, pressed)
    }

    fn messages_after_pointing_at_row(
        row_press: PointedRowPress,
        pointer_target: impl Fn(&mut Simulator<'_, GroupEvent>) -> Point,
        pressed: bool,
    ) -> Vec<RowPointerMessage> {
        let bench = MenuTestBench::default();
        let expansion = WidgetGroupExpansion::default();
        let group_menus = opened_menu_state(NestedMenuPresentation::default());
        let group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Gpu,
            GroupIndicator::glyph_only(
                WidgetGroupFamily::Gpu.title(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            &expansion,
            ClockInteraction::Expansion,
            [match row_press {
                PointedRowPress::PerformsAction => named_test_indicator("Smoke item")
                    .on_press("act", CompositorStylingMessage::ToggleXray),
                PointedRowPress::ReadOnly => named_test_indicator("Smoke item"),
            }],
        );
        let surfaces = GroupMenuView::new(GroupMenuTopic::Hardware, &group_menus, [group])
            .bench_surfaces(
                &bench,
                CascadeOrientation {
                    direction: CascadeDirection::Rightward,
                    growth: PopupGrowth::Downward,
                },
            );
        let item_list = surfaces.first_indicator_list();
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
        simulator
            .into_messages()
            .map(RowPointerMessage::from)
            .collect()
    }

    fn row_name_center(simulator: &mut Simulator<'_, GroupEvent>) -> Point {
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
                GroupIndicator::glyph_only(
                    family.title(),
                    IconGlyph::default(),
                    IndicatorState::Idle,
                ),
                &expansion,
                ClockInteraction::Expansion,
                [named_test_indicator("Smoke item")],
            )
        });
        let surfaces = GroupMenuView::new(GroupMenuTopic::Hardware, &group_menus, groups)
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
            .map(RowPointerMessage::from)
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

    fn row_icon_center(simulator: &mut Simulator<'_, GroupEvent>) -> Point {
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
                .contains(&RowPointerMessage::RowInspected(pointed_row()))
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
                .contains(&RowPointerMessage::RowInspected(pointed_row()))
        );
    }

    #[test]
    fn a_pinned_composed_indicator_draws_its_value_on_the_panel() {
        let config = Config::default();
        let clock = Clock::real();
        let mut group_menus = GroupMenuState::default();
        let _ = group_menus.update(GroupMenuMessage::TogglePin {
            menu: GroupMenuTopic::Media,
            entry: GroupMenuPin::Item(IndicatorKey {
                group: WidgetGroupIdentity::sole(WidgetGroupFamily::Media),
                name: "Lyrics".into(),
            }),
        });
        let expansion = WidgetGroupExpansion::default();
        let group = WidgetGroupContent::expandable(
            WidgetGroupFamily::Media,
            GroupIndicator::glyph_only(
                WidgetGroupFamily::Media.title(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            &expansion,
            ClockInteraction::Expansion,
            [GroupIndicator::composed_pinnable(
                "Lyrics",
                IndicatorIcon::Absent,
                IndicatorValue::drawn(|_| text("the sung line").into()),
                Box::new(|_| Vec::new()),
            )],
        );
        let panel_views: Vec<_> = GroupMenuView::new(GroupMenuTopic::Media, &group_menus, [group])
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
