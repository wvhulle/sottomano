use iced::{
    Element, Event, Point, Settings, Size,
    mouse::{Button, Event as MouseEvent},
    widget::text,
};
use iced_test::Simulator;

use crate::popovers::{
    CascadeDirection, CascadeOrientation, MenuEntry, MenuItem, MenuRow, MenuRowContent,
    MenuRowMetrics, MenuSubmenu, NestedMenuAction, NestedMenuLayout, NestedMenuPresentation,
    NestedMenuState, PopupGrowth, nested_menu,
};

const PLANETS: &str = "planets";
const JUPITER: &str = "jupiter";

type PlanetAction = NestedMenuAction<&'static str>;

fn labelled_item(label: &'static str) -> MenuEntry<'static, &'static str, PlanetAction> {
    MenuItem::new(MenuRow::new(MenuRowContent::new(label))).into()
}

fn planet_entries() -> Vec<MenuEntry<'static, &'static str, PlanetAction>> {
    vec![
        labelled_item("Sun"),
        MenuEntry::Divider,
        MenuSubmenu::new(
            PLANETS,
            MenuRowContent::new("Planets"),
            [
                labelled_item("Earth"),
                MenuSubmenu::new(
                    JUPITER,
                    MenuRowContent::new("Jupiter"),
                    [labelled_item("Io"), labelled_item("Europa")],
                )
                .into(),
            ],
        )
        .into(),
    ]
}

fn opened_state(open_path: &[&'static str]) -> NestedMenuState<&'static str> {
    let mut state = NestedMenuState::default();
    state.perform(NestedMenuAction::Opened(open_path.to_vec()));
    state
}

fn planet_menu(
    state: &NestedMenuState<&'static str>,
    layout: NestedMenuLayout,
) -> Element<'static, PlanetAction> {
    nested_menu(
        planet_entries(),
        state,
        NestedMenuPresentation {
            layout,
            ..NestedMenuPresentation::default()
        },
        |action| action,
    )
    .into()
}

fn simulated(menu: Element<'_, PlanetAction>) -> Simulator<'_, PlanetAction> {
    Simulator::with_size(Settings::default(), Size::new(900.0, 500.0), menu)
}

fn label_center(simulator: &mut Simulator<'_, PlanetAction>, label: &str) -> Point {
    simulator
        .find(label)
        .unwrap_or_else(|_| panic!("{label} is laid out"))
        .bounds()
        .center()
}

#[test]
fn hovering_a_cascading_header_reveals_its_submenu() {
    let state = NestedMenuState::default();
    let mut simulator = simulated(planet_menu(&state, NestedMenuLayout::Cascading));
    let header = label_center(&mut simulator, "Planets");
    simulator.point_at(header);
    let _moved = simulator.simulate([Event::Mouse(MouseEvent::CursorMoved { position: header })]);
    assert_eq!(
        simulator.into_messages().collect::<Vec<_>>(),
        [NestedMenuAction::Opened(vec![PLANETS])]
    );
}

#[test]
fn pressing_an_accordion_header_toggles_its_submenu() {
    let state = opened_state(&[PLANETS]);
    let mut simulator = simulated(planet_menu(&state, NestedMenuLayout::Accordion));
    let header = label_center(&mut simulator, "Planets");
    simulator.point_at(header);
    let _pressed = simulator.simulate([
        Event::Mouse(MouseEvent::CursorMoved { position: header }),
        Event::Mouse(MouseEvent::ButtonPressed(Button::Left)),
        Event::Mouse(MouseEvent::ButtonReleased(Button::Left)),
    ]);
    assert_eq!(
        simulator.into_messages().collect::<Vec<_>>(),
        [NestedMenuAction::Opened(Vec::new())]
    );
}

#[test]
fn an_open_nested_submenu_cascades_beside_its_parent_rows() {
    let state = opened_state(&[PLANETS, JUPITER]);
    let mut simulator = simulated(planet_menu(&state, NestedMenuLayout::Cascading));
    let planets = simulator
        .find("Planets")
        .expect("Planets is laid out")
        .bounds();
    let earth = simulator.find("Earth").expect("Earth is laid out").bounds();
    let jupiter = simulator
        .find("Jupiter")
        .expect("Jupiter is laid out")
        .bounds();
    let io = simulator.find("Io").expect("Io is laid out").bounds();
    assert!(earth.x > planets.x + planets.width, "{planets:?} {earth:?}");
    assert!((earth.y - planets.y).abs() < 0.5, "{planets:?} {earth:?}");
    assert!(io.x > jupiter.x + jupiter.width, "{jupiter:?} {io:?}");
    assert!((io.y - jupiter.y).abs() < 0.5, "{jupiter:?} {io:?}");
}

#[test]
fn an_open_accordion_submenu_lists_its_rows_below_the_header() {
    let state = opened_state(&[PLANETS, JUPITER]);
    let mut simulator = simulated(planet_menu(&state, NestedMenuLayout::Accordion));
    let planets = simulator
        .find("Planets")
        .expect("Planets is laid out")
        .bounds();
    let earth = simulator.find("Earth").expect("Earth is laid out").bounds();
    let io = simulator.find("Io").expect("Io is laid out").bounds();
    assert!(earth.y > planets.y, "{planets:?} {earth:?}");
    assert!(earth.x > planets.x, "{planets:?} {earth:?}");
    assert!(io.x > earth.x, "{earth:?} {io:?}");
}

fn flat_menu_label_x(entries: Vec<MenuEntry<'static, &'static str, PlanetAction>>) -> f32 {
    let state = NestedMenuState::default();
    let mut simulator = simulated(
        nested_menu(
            entries,
            &state,
            NestedMenuPresentation::default(),
            |action| action,
        )
        .into(),
    );
    simulator
        .find("Store")
        .expect("Store is laid out")
        .bounds()
        .x
}

#[test]
fn a_menu_without_icons_reserves_no_icon_column() {
    let metrics = MenuRowMetrics::default();
    let label_x = flat_menu_label_x(vec![labelled_item("Store"), labelled_item("Library")]);
    assert!(
        label_x < metrics.side_inset_px + metrics.icon_slot_px,
        "label starts at {label_x}"
    );
}

#[test]
fn one_icon_aligns_every_label_after_the_icon_column() {
    let metrics = MenuRowMetrics::default();
    let label_x = flat_menu_label_x(vec![
        labelled_item("Store"),
        MenuItem::new(MenuRow::new(MenuRowContent::new("Library").icon(text("L")))).into(),
    ]);
    assert!(
        label_x >= metrics.side_inset_px + metrics.icon_slot_px,
        "label starts at {label_x}"
    );
}

#[test]
fn leftward_submenu_headers_keep_labels_aligned_with_items() {
    let state = NestedMenuState::default();
    let leftward_menu: Element<'static, PlanetAction> = nested_menu(
        planet_entries(),
        &state,
        NestedMenuPresentation::default(),
        |action| action,
    )
    .orientation(CascadeOrientation {
        direction: CascadeDirection::Leftward,
        growth: PopupGrowth::Downward,
    })
    .into();
    let mut simulator = simulated(leftward_menu);
    let label_left_edge = |simulator: &mut Simulator<'_, PlanetAction>, label: &str| {
        simulator
            .find(label)
            .unwrap_or_else(|_| panic!("{label} is laid out"))
            .bounds()
            .x
    };
    let item_label_edge = label_left_edge(&mut simulator, "Sun");
    let header_label_edge = label_left_edge(&mut simulator, "Planets");
    assert!(
        (item_label_edge - header_label_edge).abs() < 0.5,
        "item label at {item_label_edge}, header label at {header_label_edge}"
    );
}
