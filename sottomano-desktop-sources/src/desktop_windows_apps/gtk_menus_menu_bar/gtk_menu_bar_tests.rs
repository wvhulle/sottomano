use zbus::zvariant::{Signature, StructureBuilder, Value};

use super::{
    gtk_actions_proxy::GtkActionDescription,
    gtk_menu_item_tree::{GtkMenuItems, GtkMenuKey, SECTION_LINK, SUBMENU_LINK},
    *,
};
use crate::desktop_windows_apps::application_menu_nodes::{MenuKind, MenuLabel};

fn text_value(text: &str) -> OwnedValue {
    OwnedValue::try_from(Value::from(text)).expect("text converts")
}

fn link_value(group_id: u32, menu_id: u32) -> OwnedValue {
    let link = StructureBuilder::new()
        .add_field(group_id)
        .add_field(menu_id)
        .build()
        .expect("link builds");
    OwnedValue::try_from(Value::from(link)).expect("link converts")
}

fn labelled_item(
    label: &str,
    property_name: &str,
    property_value: OwnedValue,
) -> HashMap<String, OwnedValue> {
    HashMap::from([
        ("label".to_owned(), text_value(label)),
        (property_name.to_owned(), property_value),
    ])
}

fn enabled_action() -> GtkActionDescription {
    GtkActionDescription {
        enabled: true,
        parameter_type: Signature::Unit,
        state: Vec::new(),
    }
}

fn file_menu_bar() -> GtkMenuItems {
    GtkMenuItems::from([
        (
            GtkMenuKey {
                group_id: 0,
                menu_id: 0,
            },
            vec![labelled_item("_File", SUBMENU_LINK, link_value(1, 0))],
        ),
        (
            GtkMenuKey {
                group_id: 1,
                menu_id: 0,
            },
            vec![
                HashMap::from([(SECTION_LINK.to_owned(), link_value(1, 1))]),
                HashMap::from([(SECTION_LINK.to_owned(), link_value(1, 2))]),
            ],
        ),
        (
            GtkMenuKey {
                group_id: 1,
                menu_id: 1,
            },
            vec![labelled_item("_New", "action", text_value("win.new"))],
        ),
        (
            GtkMenuKey {
                group_id: 1,
                menu_id: 2,
            },
            vec![labelled_item("_Quit", "action", text_value("app.quit"))],
        ),
    ])
}

fn action_groups() -> GtkActionGroups {
    GtkActionGroups {
        bus_name: OwnedBusName::try_from("org.example.Editor").expect("bus name"),
        application_actions: ApplicationActionPath::Exported(
            OwnedObjectPath::try_from("/org/example/Editor").expect("object path"),
        ),
        window_actions: WindowActionPath::Unresolved,
    }
}

#[test]
fn sections_are_separated_inside_a_submenu() {
    let application_actions = GtkActionDescriptions::from([("quit".to_owned(), enabled_action())]);
    let menu_bar = gtk_menu_bar(
        &file_menu_bar(),
        &application_actions,
        &GtkActionDescriptions::new(),
        action_groups(),
    );
    let [file_menu] = menu_bar.top_menus.as_slice() else {
        panic!("one top menu expected, got {:?}", menu_bar.top_menus);
    };
    assert_eq!(file_menu.label, MenuLabel::Labelled("File".to_owned()));
    let child_kinds: Vec<MenuKind> = file_menu.children.iter().map(|child| child.kind).collect();
    assert_eq!(
        child_kinds,
        [MenuKind::Standard, MenuKind::Separator, MenuKind::Standard]
    );
}

#[test]
fn window_action_without_resolved_window_is_disabled() {
    let menu_bar = gtk_menu_bar(
        &file_menu_bar(),
        &GtkActionDescriptions::from([("quit".to_owned(), enabled_action())]),
        &GtkActionDescriptions::new(),
        action_groups(),
    );
    let enabled_labels: Vec<String> = menu_bar.top_menus[0]
        .children
        .iter()
        .filter_map(|child| match &child.label {
            MenuLabel::Labelled(label) => Some(format!("{label} {:?}", child.sensitivity)),
            MenuLabel::Unlabelled => None,
        })
        .collect();
    assert_eq!(enabled_labels, ["New Disabled", "Quit Enabled"]);
}

#[test]
fn activations_name_their_scope() {
    let menu_bar = gtk_menu_bar(
        &file_menu_bar(),
        &GtkActionDescriptions::new(),
        &GtkActionDescriptions::new(),
        action_groups(),
    );
    let mut scoped_names: Vec<String> = menu_bar
        .activations
        .into_values()
        .map(|activation| format!("{} in {:?}", activation.action_name, activation.scope))
        .collect();
    scoped_names.sort_unstable();
    assert_eq!(scoped_names, ["new in Window", "quit in Application"]);
}

#[test]
fn dashed_app_id_becomes_an_underscored_object_path() {
    assert_eq!(
        application_object_path("org.gnome.text-editor"),
        "/org/gnome/text_editor"
    );
}
