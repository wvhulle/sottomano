use std::collections::HashMap;

use sottomano_desktop_sources::desktop_windows_apps::{
    application_menu_nodes::{
        MenuItemIcon, MenuItemSensitivity, MenuKind, MenuLabel, ToggleCheck, ToggleMark,
    },
    focused_window_menu_bar::{FocusedWindowOwner, MenuBarMatch, ShownMenuBar},
    gtk_menus_menu_bar::{
        ApplicationActionPath, GtkActionScope, GtkActionTarget, WindowActionPath,
    },
};
use zbus::{names::OwnedBusName, zvariant::OwnedObjectPath};

use super::*;

const FILE_MENU_ID: i32 = 1;
const QUIT_ITEM_ID: i32 = 2;
const RECENT_MENU_ID: i32 = 3;
const RECENT_FILE_ID: i32 = 4;
const SEPARATOR_ID: i32 = 5;
const PRINT_ITEM_ID: i32 = 6;
const SIDEBAR_TOGGLE_ID: i32 = 7;

fn menu_node(id: i32, label: &str, kind: MenuKind, children: Vec<MenuNode>) -> MenuNode {
    MenuNode {
        id,
        label: MenuLabel::Labelled(label.to_owned()),
        sensitivity: MenuItemSensitivity::Enabled,
        kind,
        icon: MenuItemIcon::NoIcon,
        children,
    }
}

fn file_menu() -> Vec<MenuNode> {
    vec![menu_node(
        FILE_MENU_ID,
        "File",
        MenuKind::Submenu,
        vec![
            menu_node(
                RECENT_MENU_ID,
                "Recent",
                MenuKind::Submenu,
                vec![menu_node(
                    RECENT_FILE_ID,
                    "notes.txt",
                    MenuKind::Standard,
                    Vec::new(),
                )],
            ),
            MenuNode {
                label: MenuLabel::Unlabelled,
                ..menu_node(SEPARATOR_ID, "", MenuKind::Separator, Vec::new())
            },
            MenuNode {
                sensitivity: MenuItemSensitivity::Disabled,
                ..menu_node(PRINT_ITEM_ID, "Print", MenuKind::Standard, Vec::new())
            },
            menu_node(
                SIDEBAR_TOGGLE_ID,
                "Show Sidebar",
                MenuKind::Toggle {
                    mark: ToggleMark::Checkbox,
                    check: ToggleCheck::Checked,
                },
                Vec::new(),
            ),
            menu_node(QUIT_ITEM_ID, "Quit", MenuKind::Standard, Vec::new()),
        ],
    )]
}

fn qt_menu_bar_location() -> DbusMenuLocation {
    DbusMenuLocation {
        bus_name: OwnedBusName::try_from(":1.40").expect("bus name"),
        menu_path: OwnedObjectPath::try_from("/MenuBar/1").expect("object path"),
    }
}

fn enabled_group_showing(activation_route: MenuActivationRoute) -> WindowMenuBar {
    let mut group = WindowMenuBar::default();
    let _ = group.update(WindowMenuBarMessage::UseLoaded(WindowMenuBarUse::Enabled));
    group.observe_focused_window(FocusedWindowIdentity::Window {
        window_id: 3,
        owner: FocusedWindowOwner::ProcessAndApp {
            process_id: 40,
            app_id: "org.example.Editor".to_owned(),
        },
    });
    let _ = group.update(WindowMenuBarMessage::MenuBarChanged(FocusedMenuBar::Shown(
        ShownMenuBar {
            top_menus: file_menu(),
            activation_route,
            menu_bar_match: MenuBarMatch::ExactWindow,
        },
    )));
    group
}

fn gtk_quit_route() -> MenuActivationRoute {
    MenuActivationRoute::GtkActions {
        action_groups: GtkActionGroups {
            bus_name: OwnedBusName::try_from("org.example.Editor").expect("bus name"),
            application_actions: ApplicationActionPath::Exported(
                OwnedObjectPath::try_from("/org/example/Editor").expect("object path"),
            ),
            window_actions: WindowActionPath::Unresolved,
        },
        activations: HashMap::from([(
            QUIT_ITEM_ID,
            GtkMenuActivation {
                scope: GtkActionScope::Application,
                action_name: "quit".to_owned(),
                action_target: GtkActionTarget::Untargeted,
            },
        )]),
    }
}

#[test]
fn each_labelled_top_menu_becomes_a_submenu() {
    let group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
    let top_menu_entries = group.menu_entries(&Config::default(), &IconCatalog::default());
    assert_eq!(top_menu_entries.len(), 1);
    assert!(matches!(top_menu_entries[0], MenuEntry::Submenu(_)));
    assert!(group.shows_top_menus());
}

#[test]
fn submenu_children_keep_their_nesting_and_separators() {
    let group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
    let file_children =
        group.top_menu_entries(FILE_MENU_ID, &Config::default(), &IconCatalog::default());
    assert!(matches!(
        file_children.as_slice(),
        [
            MenuEntry::Submenu(_),
            MenuEntry::Divider,
            MenuEntry::Item(_),
            MenuEntry::Item(_),
            MenuEntry::Item(_)
        ]
    ));
}

#[test]
fn unknown_top_menu_id_has_no_entries() {
    let group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
    assert!(
        group
            .top_menu_entries(QUIT_ITEM_ID, &Config::default(), &IconCatalog::default())
            .is_empty()
    );
}

#[test]
fn inline_layout_shows_top_menus_in_the_panel() {
    let mut group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
    assert!(group.shows_inline_top_menus());
    let _ = group.interact(WindowMenuBarInteraction::ToggleLayout);
    assert_eq!(
        group.window_menu_bar_layout,
        WindowMenuBarLayout::BehindHandle
    );
    assert!(!group.shows_inline_top_menus());
    let _ = group.interact(WindowMenuBarInteraction::ToggleLayout);
    assert!(group.shows_inline_top_menus());
    let _ = group.interact(WindowMenuBarInteraction::ToggleUse);
    assert!(!group.shows_inline_top_menus());
}

#[test]
fn switched_off_group_shows_nothing() {
    let mut group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
    let _ = group.interact(WindowMenuBarInteraction::ToggleUse);
    assert_eq!(group.window_menu_bar_use, WindowMenuBarUse::Disabled);
    assert!(
        group
            .menu_entries(&Config::default(), &IconCatalog::default())
            .is_empty()
    );
    assert!(!group.shows_top_menus());
}

#[test]
fn pressing_an_entry_closes_the_open_submenus() {
    let mut group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
    let _ = group.interact(WindowMenuBarInteraction::Menu(NestedMenuAction::Opened(
        vec![FILE_MENU_ID, RECENT_MENU_ID],
    )));
    let _ = group.interact(WindowMenuBarInteraction::ItemPressed(RECENT_FILE_ID));
    assert!(group.menu_state.open_submenus.is_empty());
}

#[test]
fn group_starts_switched_on_with_inline_top_menus() {
    let group = WindowMenuBar::default();
    assert_eq!(group.window_menu_bar_use, WindowMenuBarUse::Enabled);
    assert_eq!(
        group.window_menu_bar_layout,
        WindowMenuBarLayout::InlineTopMenus
    );
}

#[test]
fn pressing_a_qt_entry_clicks_it_by_id() {
    let mut group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
    assert!(matches!(
        group.interact(WindowMenuBarInteraction::ItemPressed(QUIT_ITEM_ID)),
        Action::Activate {
            request: WindowMenuBarRequest::ClickDbusMenuItem {
                item_id: QUIT_ITEM_ID,
                ..
            },
            dismissal: PressDismissal::ClosesMenu,
        }
    ));
}

#[test]
fn pressing_a_toggle_keeps_the_menu_open() {
    let mut group = enabled_group_showing(MenuActivationRoute::DbusMenu(qt_menu_bar_location()));
    let opened_path = vec![FILE_MENU_ID];
    let _ = group.interact(WindowMenuBarInteraction::Menu(NestedMenuAction::Opened(
        opened_path.clone(),
    )));
    assert!(matches!(
        group.interact(WindowMenuBarInteraction::ItemPressed(SIDEBAR_TOGGLE_ID)),
        Action::Activate {
            dismissal: PressDismissal::KeepsMenuOpen,
            ..
        }
    ));
    assert_eq!(group.menu_state.open_submenus, opened_path);
}

#[test]
fn pressing_a_gtk_entry_activates_its_action() {
    let mut group = enabled_group_showing(gtk_quit_route());
    let Action::Activate {
        request: WindowMenuBarRequest::ActivateGtkAction { activation, .. },
        ..
    } = group.interact(WindowMenuBarInteraction::ItemPressed(QUIT_ITEM_ID))
    else {
        panic!("the quit entry activates a gtk action");
    };
    assert_eq!(activation.action_name, "quit");
}

#[test]
fn menu_bar_survives_the_panel_taking_keyboard_focus() {
    let mut group = enabled_group_showing(gtk_quit_route());
    group.observe_focused_window(FocusedWindowIdentity::NoFocusedWindow);
    assert!(group.shows_top_menus());
    assert_eq!(
        group
            .menu_entries(&Config::default(), &IconCatalog::default())
            .len(),
        1
    );
}

#[test]
fn focus_change_drops_the_previous_menu_bar() {
    let mut group = enabled_group_showing(gtk_quit_route());
    group.observe_focused_window(FocusedWindowIdentity::Window {
        window_id: 9,
        owner: FocusedWindowOwner::ProcessOnly { process_id: 9 },
    });
    assert!(
        group
            .menu_entries(&Config::default(), &IconCatalog::default())
            .is_empty()
    );
}
