use corredo::popovers::{
    CascadeDirection, CascadeOrientation, NestedMenuAction, NestedMenuLayout,
    NestedMenuOpenTrigger, NestedMenuPresentation, NestedMenuSurfaces, OpenGroupLimit, PopupGrowth,
    SubmenuConnector,
};
use iced::{Element, Settings, Size};
use iced_controller_diagram::ControllerArtwork;
use iced_test::Simulator;
use sottomano_toml_config::Config;

use super::{GroupEvent, GroupMenuView, InspectionCard};
use crate::{
    clock::Clock,
    desktop_index::DesktopIndex,
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue, named_test_indicator},
        indicator_drawing::IndicatorIcon,
        indicator_state::IndicatorState,
    },
    group_menus::{
        group_menu_state::{GroupMenuMessage, GroupMenuState, GroupMenuTopic},
        menu_rows::menu_row::menu_metrics,
    },
    hover_cards::{
        IndicatorKey, MouseGesture, PopupMessage, PressAction,
        hover_card_body::{Section, SectionWidget},
    },
    indicator_groups::{
        clock_group::ClockInteraction,
        indicator_group_content::{IndicatorGroupContent, IndicatorGroupExpansion},
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
    widgets::{
        favicon::Favicons, icon_glyph::IconGlyph, shell_layer_appearance::ShellLayer,
        window_icon::WindowAssets,
    },
};

pub(super) struct MenuTestBench {
    config: Config,
    sections: Vec<Section>,
    pub(super) entries: DesktopIndex,
    pub(super) favicons: Favicons,
    pub(super) controller_artwork: ControllerArtwork,
    pub(super) clock: Clock,
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
    pub(super) fn inspection_card(&self) -> InspectionCard<'_, '_> {
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
    pub(super) fn bench_surfaces(
        self,
        bench: &'a MenuTestBench,
        orientation: CascadeOrientation,
    ) -> NestedMenuSurfaces<'a, GroupEvent> {
        self.surfaces(
            &bench.config,
            &ShellLayer::Overlays.default_appearance(),
            &bench.inspection_card(),
            orientation,
        )
        .expect("an opened menu has a surface")
    }
}

pub(super) fn first_indicator_list(
    surfaces: NestedMenuSurfaces<'_, GroupEvent>,
) -> Element<'_, GroupEvent> {
    surfaces
        .satellites
        .into_iter()
        .next()
        .expect("the open group has an item list")
}

pub(super) fn every_presentation() -> Vec<NestedMenuPresentation> {
    let layouts = [NestedMenuLayout::Cascading, NestedMenuLayout::Accordion];
    let triggers = [NestedMenuOpenTrigger::Hover, NestedMenuOpenTrigger::Press];
    let limits = [OpenGroupLimit::One, OpenGroupLimit::Several];
    let connectors = [SubmenuConnector::Hidden, SubmenuConnector::Shown];
    layouts
        .into_iter()
        .flat_map(|layout| {
            triggers.into_iter().flat_map(move |open_trigger| {
                limits.into_iter().flat_map(move |accordion_open_limit| {
                    connectors
                        .into_iter()
                        .map(move |submenu_connector| NestedMenuPresentation {
                            layout,
                            open_trigger,
                            accordion_open_limit,
                            submenu_connector,
                        })
                })
            })
        })
        .collect()
}

pub(super) fn opened_menu_state(presentation: NestedMenuPresentation) -> GroupMenuState {
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(GroupMenuMessage::PresentationLoaded(presentation));
    let _ = group_menus.update(GroupMenuMessage::Menu {
        menu: GroupMenuTopic::Hardware,
        action: NestedMenuAction::Opened(vec![IndicatorGroupIdentity::sole(
            IndicatorGroupFamily::Gpu,
        )]),
    });
    group_menus
}

pub(super) fn row_tops(
    group_menus: &GroupMenuState,
    orientation: CascadeOrientation,
    clock: Clock,
) -> (f32, Option<f32>) {
    let bench = MenuTestBench {
        clock,
        ..MenuTestBench::default()
    };
    let expansion = IndicatorGroupExpansion::default();
    let group = IndicatorGroupContent::expandable(
        IndicatorGroupFamily::Gpu,
        GroupIndicator::glyph_only(
            IndicatorGroupFamily::Gpu.title(),
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
        .find(IndicatorGroupFamily::Gpu.title())
        .expect("the group row is on screen")
        .visible_bounds()
        .expect("the group row is visible")
        .y;
    let indicator_top = match primary.find("Smoke item") {
        Err(iced_test::Error::SelectorNotFound { .. }) => None,
        found => found.unwrap().visible_bounds().map(|bounds| bounds.y),
    };
    (header_top, indicator_top)
}

#[test]
fn a_group_without_indicators_leaves_the_menu() {
    let group_menus = opened_menu_state(every_presentation()[0]);
    let expansion = IndicatorGroupExpansion::default();
    let empty_group = IndicatorGroupContent::expandable(
        IndicatorGroupFamily::Gpu,
        GroupIndicator::glyph_only(
            IndicatorGroupFamily::Gpu.title(),
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
    let expansion = IndicatorGroupExpansion::default();
    let headline_group = IndicatorGroupContent::expandable(
        IndicatorGroupFamily::Gpu,
        named_test_indicator("Headline").with_headline_glyph(IconGlyph::default()),
        &expansion,
        ClockInteraction::Expansion,
        [named_test_indicator("Listed")],
    );
    let indicator_key = |name: &str| IndicatorKey {
        group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Gpu),
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

pub(super) fn long_indicator_list_row_top(
    screen_height: f32,
    item_list_scrolled_px: f32,
    row_name: &str,
) -> f32 {
    let expansion = IndicatorGroupExpansion::default();
    let long_list = (0..40).map(|position| {
        GroupIndicator::composed(
            format!("Row {position}"),
            IndicatorIcon::Absent,
            IndicatorValue::Nothing,
            Box::new(|_| Vec::new()),
        )
    });
    let group = IndicatorGroupContent::expandable(
        IndicatorGroupFamily::Media,
        GroupIndicator::glyph_only(
            IndicatorGroupFamily::Media.title(),
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
    let _ = group_menus.update(GroupMenuMessage::Menu {
        menu: GroupMenuTopic::Media,
        action: NestedMenuAction::Opened(vec![IndicatorGroupIdentity::sole(
            IndicatorGroupFamily::Media,
        )]),
    });
    let _ = group_menus.update(GroupMenuMessage::Menu {
        menu: GroupMenuTopic::Media,
        action: NestedMenuAction::Scrolled {
            depth: 1,
            scrolled_px: item_list_scrolled_px,
        },
    });
    let bench = MenuTestBench::default();
    let surfaces = GroupMenuView::new(GroupMenuTopic::Media, &group_menus, [group]).bench_surfaces(
        &bench,
        CascadeOrientation {
            direction: CascadeDirection::Rightward,
            growth: PopupGrowth::Downward,
        },
    );
    let item_list = first_indicator_list(surfaces);
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
    let row_pitch = menu_metrics(&Config::default()).row_height_px;
    let screen_height = row_pitch * 40.0;
    let unscrolled = long_indicator_list_row_top(screen_height, 0.0, "Row 0");
    let scrolled_past_the_end = long_indicator_list_row_top(screen_height, f32::MAX, "Row 0");
    assert!((scrolled_past_the_end - unscrolled).abs() < 1.0);
}

#[test]
fn an_indicator_list_taller_than_the_screen_scrolls_to_its_last_rows() {
    let row_pitch = menu_metrics(&Config::default()).row_height_px;
    let screen_height = row_pitch * 10.0;
    let first_row_top = long_indicator_list_row_top(screen_height, 0.0, "Row 0");
    let first_row_of_the_last_page = long_indicator_list_row_top(screen_height, f32::MAX, "Row 30");
    assert!((first_row_of_the_last_page - first_row_top).abs() < 1.0);
}

pub(super) fn inspection_card_view(
    sections: Vec<Section>,
) -> Option<Element<'static, PopupMessage>> {
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

#[test]
fn a_card_taller_than_the_screen_keeps_its_rows_apart() {
    let tall_sections = (0..40)
        .map(|position| {
            Section::new(
                format!("Deadline {position}"),
                SectionWidget::paragraph(format!("task {position}")),
            )
        })
        .collect();
    let card = inspection_card_view(tall_sections).expect("a card with sections is shown");
    let mut simulator = Simulator::with_size(Settings::default(), Size::new(600.0, 300.0), card);
    let title_tops: Vec<f32> = (0..40)
        .map(|position| {
            simulator
                .find(format!("Deadline {position}"))
                .expect("the title is laid out")
                .bounds()
                .y
        })
        .collect();
    let line_height = menu_metrics(&Config::default()).row_height_px / 2.0;
    assert!(
        title_tops
            .windows(2)
            .all(|neighbours| neighbours[1] - neighbours[0] >= line_height)
    );
}
