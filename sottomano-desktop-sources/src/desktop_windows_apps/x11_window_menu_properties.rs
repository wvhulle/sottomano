use std::collections::HashMap;

use x11rb::{
    connection::Connection as _,
    errors::{ConnectError, ConnectionError, ReplyError},
    protocol::xproto::{AtomEnum, ConnectionExt},
    rust_connection::RustConnection,
};
use zbus::{names::OwnedBusName, zvariant::OwnedObjectPath};

use crate::{
    blocking_background_work::{Panicked, blocking_poll},
    desktop_windows_apps::{
        dbusmenu_menu_bar_layout::DbusMenuLocation,
        gtk_menus_menu_bar::{
            ApplicationActionPath, GtkActionGroups, GtkMenuBarSource, WindowActionPath,
        },
    },
};

const ACTIVE_WINDOW_PROPERTY: &str = "_NET_ACTIVE_WINDOW";
const KDE_SERVICE_PROPERTY: &str = "_KDE_NET_WM_APPMENU_SERVICE_NAME";
const KDE_OBJECT_PATH_PROPERTY: &str = "_KDE_NET_WM_APPMENU_OBJECT_PATH";
const GTK_BUS_NAME_PROPERTY: &str = "_GTK_UNIQUE_BUS_NAME";
const GTK_MENUBAR_PATH_PROPERTY: &str = "_GTK_MENUBAR_OBJECT_PATH";
const GTK_APPLICATION_PATH_PROPERTY: &str = "_GTK_APPLICATION_OBJECT_PATH";
const GTK_WINDOW_PATH_PROPERTY: &str = "_GTK_WINDOW_OBJECT_PATH";
const MENU_PROPERTIES: [&str; 6] = [
    KDE_SERVICE_PROPERTY,
    KDE_OBJECT_PATH_PROPERTY,
    GTK_BUS_NAME_PROPERTY,
    GTK_MENUBAR_PATH_PROPERTY,
    GTK_APPLICATION_PATH_PROPERTY,
    GTK_WINDOW_PATH_PROPERTY,
];
const PROPERTY_LENGTH_WORDS: u32 = 1024;

#[derive(Debug, thiserror::Error)]
pub enum X11MenuLookupError {
    #[error("x11 connect: {0}")]
    Connect(#[from] ConnectError),
    #[error("x11 request: {0}")]
    Request(#[from] ConnectionError),
    #[error("x11 reply: {0}")]
    Reply(#[from] ReplyError),
    #[error("x11 lookup: {0}")]
    Panicked(#[from] Panicked),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveX11Window {
    Window(u32),
    NoneActive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X11WindowMenu {
    KdeDbusMenu(DbusMenuLocation),
    GtkMenus(GtkMenuBarSource),
    Unpublished,
}

fn atom_named(connection: &RustConnection, atom_name: &str) -> Result<u32, X11MenuLookupError> {
    Ok(connection
        .intern_atom(false, atom_name.as_bytes())?
        .reply()?
        .atom)
}

fn read_active_window() -> Result<ActiveX11Window, X11MenuLookupError> {
    let (connection, screen_number) = x11rb::connect(None)?;
    let root_window = connection.setup().roots[screen_number].root;
    let active_atom = atom_named(&connection, ACTIVE_WINDOW_PROPERTY)?;
    let active_reply = connection
        .get_property(false, root_window, active_atom, AtomEnum::WINDOW, 0, 1)?
        .reply()?;
    let active_window = active_reply
        .value32()
        .and_then(|mut window_ids| window_ids.next())
        .unwrap_or(0);
    Ok(match active_window {
        0 => ActiveX11Window::NoneActive,
        window_id => ActiveX11Window::Window(window_id),
    })
}

fn read_menu_properties(
    window_id: u32,
) -> Result<HashMap<&'static str, String>, X11MenuLookupError> {
    let (connection, _) = x11rb::connect(None)?;
    let mut property_values = HashMap::new();
    for property_name in MENU_PROPERTIES {
        let property_atom = atom_named(&connection, property_name)?;
        let property_reply = connection
            .get_property(
                false,
                window_id,
                property_atom,
                AtomEnum::ANY,
                0,
                PROPERTY_LENGTH_WORDS,
            )?
            .reply()?;
        let property_text = String::from_utf8_lossy(&property_reply.value)
            .trim_end_matches('\0')
            .to_owned();
        if !property_text.is_empty() {
            property_values.insert(property_name, property_text);
        }
    }
    Ok(property_values)
}

fn kde_dbus_menu(property_values: &HashMap<&'static str, String>) -> Option<DbusMenuLocation> {
    let bus_name =
        OwnedBusName::try_from(property_values.get(KDE_SERVICE_PROPERTY)?.as_str()).ok()?;
    let menu_path =
        OwnedObjectPath::try_from(property_values.get(KDE_OBJECT_PATH_PROPERTY)?.as_str()).ok()?;
    Some(DbusMenuLocation {
        bus_name,
        menu_path,
    })
}

fn gtk_menu_bar_source(
    property_values: &HashMap<&'static str, String>,
) -> Option<GtkMenuBarSource> {
    let bus_name =
        OwnedBusName::try_from(property_values.get(GTK_BUS_NAME_PROPERTY)?.as_str()).ok()?;
    let menubar_path =
        OwnedObjectPath::try_from(property_values.get(GTK_MENUBAR_PATH_PROPERTY)?.as_str()).ok()?;
    let object_path_of = |property_name| {
        property_values
            .get(property_name)
            .and_then(|object_path| OwnedObjectPath::try_from(object_path.as_str()).ok())
    };
    let application_actions = object_path_of(GTK_APPLICATION_PATH_PROPERTY).map_or(
        ApplicationActionPath::Unexported,
        ApplicationActionPath::Exported,
    );
    let window_actions = object_path_of(GTK_WINDOW_PATH_PROPERTY)
        .map_or(WindowActionPath::Unresolved, WindowActionPath::Resolved);
    Some(GtkMenuBarSource {
        menubar_path,
        action_groups: GtkActionGroups {
            bus_name,
            application_actions,
            window_actions,
        },
    })
}

fn window_menu_of_properties(property_values: &HashMap<&'static str, String>) -> X11WindowMenu {
    if let Some(location) = kde_dbus_menu(property_values) {
        return X11WindowMenu::KdeDbusMenu(location);
    }
    gtk_menu_bar_source(property_values).map_or(X11WindowMenu::Unpublished, X11WindowMenu::GtkMenus)
}

pub async fn active_x11_window() -> Result<ActiveX11Window, X11MenuLookupError> {
    blocking_poll(read_active_window).await
}

pub async fn x11_window_menu(window_id: u32) -> Result<X11WindowMenu, X11MenuLookupError> {
    blocking_poll(move || read_menu_properties(window_id))
        .await
        .map(|property_values| window_menu_of_properties(&property_values))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn properties(property_pairs: &[[&'static str; 2]]) -> HashMap<&'static str, String> {
        property_pairs
            .iter()
            .map(|[property_name, property_value]| (*property_name, (*property_value).to_owned()))
            .collect()
    }

    #[test]
    fn kde_properties_name_the_dbusmenu_object() {
        let window_menu = window_menu_of_properties(&properties(&[
            [KDE_SERVICE_PROPERTY, ":1.52"],
            [KDE_OBJECT_PATH_PROPERTY, "/MenuBar/1"],
        ]));
        assert_eq!(
            window_menu,
            X11WindowMenu::KdeDbusMenu(DbusMenuLocation {
                bus_name: OwnedBusName::try_from(":1.52").expect("bus name"),
                menu_path: OwnedObjectPath::try_from("/MenuBar/1").expect("object path"),
            })
        );
    }

    #[test]
    fn gtk_properties_give_exact_action_paths() {
        let window_menu = window_menu_of_properties(&properties(&[
            [GTK_BUS_NAME_PROPERTY, ":1.60"],
            [
                GTK_MENUBAR_PATH_PROPERTY,
                "/org/example/Editor/menus/menubar",
            ],
            [GTK_APPLICATION_PATH_PROPERTY, "/org/example/Editor"],
            [GTK_WINDOW_PATH_PROPERTY, "/org/example/Editor/window/3"],
        ]));
        let X11WindowMenu::GtkMenus(source) = window_menu else {
            panic!("gtk properties describe a gtk menu bar, got {window_menu:?}");
        };
        assert_eq!(
            source.action_groups.window_actions,
            WindowActionPath::Resolved(
                OwnedObjectPath::try_from("/org/example/Editor/window/3").expect("object path")
            )
        );
    }

    #[test]
    fn gtk_menu_bar_without_application_keeps_window_actions() {
        let window_menu = window_menu_of_properties(&properties(&[
            [GTK_BUS_NAME_PROPERTY, ":1.61"],
            [
                GTK_MENUBAR_PATH_PROPERTY,
                "/com/canonical/unity/gtk/window/0",
            ],
            [
                GTK_WINDOW_PATH_PROPERTY,
                "/com/canonical/unity/gtk/window/0",
            ],
        ]));
        let X11WindowMenu::GtkMenus(source) = window_menu else {
            panic!("gtk properties describe a gtk menu bar, got {window_menu:?}");
        };
        assert_eq!(
            source.action_groups.application_actions,
            ApplicationActionPath::Unexported
        );
    }

    #[test]
    fn half_published_kde_properties_fall_back_to_gtk() {
        let window_menu = window_menu_of_properties(&properties(&[
            [KDE_SERVICE_PROPERTY, ":1.52"],
            [GTK_BUS_NAME_PROPERTY, ":1.60"],
            [
                GTK_MENUBAR_PATH_PROPERTY,
                "/org/example/Editor/menus/menubar",
            ],
        ]));
        assert!(matches!(window_menu, X11WindowMenu::GtkMenus(_)));
    }

    #[test]
    fn window_without_menu_properties_publishes_nothing() {
        assert_eq!(
            window_menu_of_properties(&HashMap::new()),
            X11WindowMenu::Unpublished
        );
    }
}
