use corredo::popovers::{
    CascadeDirection, CascadeOrientation, NestedMenuAction, NestedMenuPresentation, PopupGrowth,
};
use iced::{
    Event, Point, Settings, Size, mouse,
    widget::{Row, text},
    window,
};
use iced_test::{Simulator, simulator::click};
use sottomano_toml_config::{Config, screen_edges::Position};

use super::{
    GroupEvent, GroupMenuView,
    group_menu_layout_tests::{MenuTestBench, first_indicator_list, opened_menu_state},
};
use crate::{
    clock::Clock,
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{
        group_indicator::{GroupIndicator, IndicatorValue, named_test_indicator},
        indicator_drawing::IndicatorIcon,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorListing,
        },
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
        indicator_group_interaction::IndicatorGroupInteraction,
        indicator_state::IndicatorState,
    },
    group_menus::group_menu_state::{
        GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
    },
    hover_cards::{IndicatorKey, MenuHandleIndicator, hover_card_surfaces::SWITCH_INTENT},
    panel_menus::{
        agenda_menu::clock_group::ClockInteraction,
        appearance_menu::compositor_styling_group::CompositorStylingMessage,
    },
};

#[derive(Debug, Clone, PartialEq)]
enum RowPointerMessage {
    RowInspected(IndicatorKey),
    RowAction,
    Revealed(IndicatorGroupIdentity),
    Unrelated,
}

impl From<GroupEvent> for RowPointerMessage {
    fn from(event: GroupEvent) -> Self {
        match event {
            GroupEvent::Interaction(IndicatorGroupInteraction::CompositorStyling(
                CompositorStylingMessage::ToggleXray,
            )) => RowPointerMessage::RowAction,
            GroupEvent::Interaction(IndicatorGroupInteraction::GroupMenus(menu_message)) => {
                match menu_message {
                    GroupMenuMessage::InspectRow { indicator, .. } => {
                        RowPointerMessage::RowInspected(indicator)
                    }
                    GroupMenuMessage::Menu {
                        action: NestedMenuAction::Opened(open_path),
                        ..
                    } => match open_path.last() {
                        Some(group_identity) => RowPointerMessage::Revealed(group_identity.clone()),
                        None => RowPointerMessage::Unrelated,
                    },
                    _ => RowPointerMessage::Unrelated,
                }
            }
            GroupEvent::Interaction(_) | GroupEvent::Popup(_) => RowPointerMessage::Unrelated,
        }
    }
}

fn pointed_row() -> IndicatorKey {
    IndicatorKey {
        group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Gpu),
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
    let expansion = IndicatorGroupExpansion::default();
    let group_menus = opened_menu_state(NestedMenuPresentation::default());
    let group = IndicatorGroupContent::expandable(
        IndicatorGroupFamily::Gpu,
        GroupIndicator::glyph_only(
            IndicatorGroupFamily::Gpu.title(),
            IconGlyph::default(),
            IndicatorState::Idle,
        ),
        &expansion,
        ClockInteraction::Expansion,
        IndicatorListing::Listed(Vec::from([match row_press {
            PointedRowPress::PerformsAction => named_test_indicator("Smoke item")
                .on_press("act", CompositorStylingMessage::ToggleXray),
            PointedRowPress::ReadOnly => named_test_indicator("Smoke item"),
        }])),
    );
    let surfaces = GroupMenuView::new(GroupMenuTopic::Hardware, &group_menus, [group])
        .bench_surfaces(
            &bench,
            CascadeOrientation {
                direction: CascadeDirection::Rightward,
                growth: PopupGrowth::Downward,
            },
        );
    let item_list = first_indicator_list(surfaces);
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

#[derive(Clone, Copy)]
enum PointerAfterwards {
    LeavesTheRow,
    RestsPastTheRevealDelay,
}

fn compute_reveal_messages(
    approach: PointerApproach,
    afterwards: PointerAfterwards,
) -> Vec<RowPointerMessage> {
    let bench = MenuTestBench::default();
    let expansion = IndicatorGroupExpansion::default();
    let group_menus = opened_menu_state(NestedMenuPresentation::default());
    let groups = [IndicatorGroupFamily::Gpu, IndicatorGroupFamily::Compute].map(|family| {
        IndicatorGroupContent::expandable(
            family,
            GroupIndicator::glyph_only(family.title(), IconGlyph::default(), IndicatorState::Idle),
            &expansion,
            ClockInteraction::Expansion,
            IndicatorListing::Listed(Vec::from([named_test_indicator("Smoke item")])),
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
    [approach_start, row_center]
        .into_iter()
        .for_each(|pointer| {
            simulator.point_at(pointer);
            let _statuses = simulator.simulate([Event::Mouse(mouse::Event::CursorMoved {
                position: pointer,
            })]);
        });
    let _statuses = match afterwards {
        PointerAfterwards::LeavesTheRow => {
            let outside = Point::new(1190.0, 790.0);
            simulator.point_at(outside);
            simulator.simulate([Event::Mouse(mouse::Event::CursorMoved {
                position: outside,
            })])
        }
        PointerAfterwards::RestsPastTheRevealDelay => {
            let entered_at = Clock::real().instant;
            simulator.simulate([
                Event::Window(window::Event::RedrawRequested(entered_at)),
                Event::Window(window::Event::RedrawRequested(
                    entered_at + 2 * SWITCH_INTENT,
                )),
            ])
        }
    };
    let compute = IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute);
    simulator
        .into_messages()
        .map(RowPointerMessage::from)
        .filter(|message| *message == RowPointerMessage::Revealed(compute.clone()))
        .collect()
}

#[test]
fn pointing_straight_down_onto_a_group_row_reveals_it_at_once() {
    let compute = IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute);
    assert_eq!(
        compute_reveal_messages(
            PointerApproach::StraightDown,
            PointerAfterwards::LeavesTheRow
        ),
        [RowPointerMessage::Revealed(compute)]
    );
}

#[test]
fn crossing_a_group_row_toward_the_open_satellite_defers_its_reveal() {
    assert_eq!(
        compute_reveal_messages(
            PointerApproach::TowardTheSatellite,
            PointerAfterwards::LeavesTheRow
        ),
        []
    );
}

#[test]
fn resting_on_a_crossed_group_row_reveals_it_after_the_delay() {
    let compute = IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute);
    assert_eq!(
        compute_reveal_messages(
            PointerApproach::TowardTheSatellite,
            PointerAfterwards::RestsPastTheRevealDelay
        ),
        [RowPointerMessage::Revealed(compute)]
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
    assert!(messages_after_pointing(row_icon_center, true).contains(&RowPointerMessage::RowAction));
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
    assert!(messages_after_pointing(row_name_center, true).contains(&RowPointerMessage::RowAction));
}

#[test]
fn pressing_a_read_only_row_name_sends_no_row_message() {
    let messages = messages_after_pointing_at_row(PointedRowPress::ReadOnly, row_name_center, true);
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
            group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Media),
            name: "Lyrics".into(),
        }),
    });
    let expansion = IndicatorGroupExpansion::default();
    let group = IndicatorGroupContent::expandable(
        IndicatorGroupFamily::Media,
        GroupIndicator::glyph_only(
            IndicatorGroupFamily::Media.title(),
            IconGlyph::default(),
            IndicatorState::Idle,
        ),
        &expansion,
        ClockInteraction::Expansion,
        IndicatorListing::Listed(Vec::from([GroupIndicator::composed_pinnable(
            "Lyrics",
            IndicatorIcon::Absent,
            IndicatorValue::drawn(|_| text("the sung line").into()),
            Box::new(|_| Vec::new()),
        )])),
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
