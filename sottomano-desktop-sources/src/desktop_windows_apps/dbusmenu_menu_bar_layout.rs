use std::collections::HashMap;

use futures::{Stream, StreamExt, future::join_all, stream};
use serde::{Deserialize, Serialize};
use system_tray::menu::MenuItem;
use zbus::{
    fdo::{DBusProxy, IntrospectableProxy},
    names::{BusName, OwnedBusName, OwnedUniqueName},
    proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, OwnedValue, Type, Value},
};

use crate::{
    delivery_cadence_drivers::ChangeTriggerStreams,
    desktop_windows_apps::{
        application_menu_nodes::{MenuNode, visible_menu_node},
        desktop_entry_app_icons::DesktopEntries,
    },
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
};

const DBUSMENU_INTERFACE: &str = "com.canonical.dbusmenu";
const QT_MENU_BAR_ROOT: &str = "/MenuBar";
const WHOLE_TREE_DEPTH: i32 = -1;
const ROOT_MENU_ITEM: i32 = 0;

struct NumberedMenuBar {
    bar_number: u32,
    object_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DbusMenuLocation {
    pub bus_name: OwnedBusName,
    pub menu_path: OwnedObjectPath,
}

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct MenuLayoutReply {
    revision: u32,
    root: MenuLayoutRoot,
}

#[derive(Debug, Deserialize, Serialize, Type)]
struct MenuLayoutRoot {
    id: i32,
    properties: HashMap<String, OwnedValue>,
    children: Vec<OwnedValue>,
}

#[proxy(interface = "com.canonical.dbusmenu", assume_defaults = false)]
pub trait DbusMenu {
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: &[&str],
    ) -> zbus::Result<MenuLayoutReply>;

    fn event(&self, id: i32, event_id: &str, data: &Value<'_>, timestamp: u32) -> zbus::Result<()>;

    fn about_to_show(&self, id: i32) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn layout_updated(&self, revision: u32, parent: i32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn items_properties_updated(
        &self,
        updated_props: Vec<OwnedValue>,
        removed_props: Vec<OwnedValue>,
    ) -> zbus::Result<()>;
}

pub async fn dbusmenu_proxy<'proxy>(
    connection: &zbus::Connection,
    location: &DbusMenuLocation,
) -> zbus::Result<DbusMenuProxy<'proxy>> {
    DbusMenuProxy::builder(connection)
        .destination(location.bus_name.clone())?
        .path(location.menu_path.clone())?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

async fn menu_bar_top_menus(menu_proxy: &DbusMenuProxy<'_>) -> zbus::Result<Vec<MenuNode>> {
    let layout = menu_proxy
        .get_layout(ROOT_MENU_ITEM, WHOLE_TREE_DEPTH, &[])
        .await?;
    let desktop_entries = DesktopEntries::empty();
    Ok(layout
        .root
        .children
        .iter()
        .filter_map(|child_value| MenuItem::try_from(child_value).ok())
        .filter_map(|menu_item| visible_menu_node(&menu_item, &desktop_entries))
        .collect())
}

pub async fn menu_bar_layout_updates(
    connection: zbus::Connection,
    location: DbusMenuLocation,
) -> zbus::Result<impl Stream<Item = zbus::Result<Vec<MenuNode>>> + Send> {
    let menu_proxy = dbusmenu_proxy(&connection, &location).await?;
    let layout_changes = menu_proxy.receive_layout_updated().await?.map(drop);
    let property_changes = menu_proxy
        .receive_items_properties_updated()
        .await?
        .map(drop);
    Ok(
        stream::select(layout_changes, property_changes).triggered_rereads(move || {
            let menu_proxy = menu_proxy.clone();
            async move { menu_bar_top_menus(&menu_proxy).await }
        }),
    )
}

pub async fn exported_menu_bars_of_process(
    connection: &zbus::Connection,
    process_id: u32,
) -> zbus::Result<Vec<DbusMenuLocation>> {
    let dbus = DBusProxy::new(connection).await?;
    let unique_names: Vec<OwnedUniqueName> = dbus
        .list_names()
        .await?
        .into_iter()
        .filter_map(|bus_name| match bus_name.into_inner() {
            BusName::Unique(unique_name) => Some(OwnedUniqueName::from(unique_name)),
            BusName::WellKnown(_) => None,
        })
        .collect();
    let process_names: Vec<OwnedUniqueName> =
        join_all(unique_names.into_iter().map(|unique_name| {
            let dbus = &dbus;
            async move {
                let owner = OwnerProcess::of_bus(dbus, BusName::Unique(unique_name.as_ref())).await;
                (owner == OwnerProcess::Identified(process_id)).then_some(unique_name)
            }
        }))
        .await
        .into_iter()
        .flatten()
        .collect();
    let mut exported_locations = Vec::new();
    for unique_name in process_names {
        exported_locations.extend(qt_menu_bars_on(connection, unique_name).await);
    }
    Ok(exported_locations)
}

async fn qt_menu_bars_on(
    connection: &zbus::Connection,
    bus_name: OwnedUniqueName,
) -> Vec<DbusMenuLocation> {
    let Ok(menu_bar_root) = introspected_node(
        connection,
        &BusName::Unique(bus_name.as_ref()),
        QT_MENU_BAR_ROOT,
    )
    .await
    else {
        return Vec::new();
    };
    let mut numbered_menu_bars: Vec<NumberedMenuBar> = menu_bar_root
        .nodes()
        .iter()
        .filter_map(|child_node| child_node.name())
        .filter_map(|child_name| {
            child_name
                .parse::<u32>()
                .ok()
                .map(|bar_number| NumberedMenuBar {
                    bar_number,
                    object_path: format!("{QT_MENU_BAR_ROOT}/{child_name}"),
                })
        })
        .collect();
    numbered_menu_bars.sort_unstable_by_key(|numbered_menu_bar| numbered_menu_bar.bar_number);
    let mut exported_locations = Vec::new();
    for NumberedMenuBar { object_path, .. } in numbered_menu_bars {
        let Ok(child_node) = introspected_node(
            connection,
            &BusName::Unique(bus_name.as_ref()),
            &object_path,
        )
        .await
        else {
            continue;
        };
        let exports_dbusmenu = child_node
            .interfaces()
            .iter()
            .any(|interface| interface.name() == DBUSMENU_INTERFACE);
        let Ok(menu_path) = OwnedObjectPath::try_from(object_path) else {
            continue;
        };
        if exports_dbusmenu {
            exported_locations.push(DbusMenuLocation {
                bus_name: OwnedBusName::from(BusName::Unique(bus_name.as_ref())),
                menu_path,
            });
        }
    }
    exported_locations
}

pub(crate) async fn introspected_node(
    connection: &zbus::Connection,
    bus_name: &BusName<'_>,
    object_path: &str,
) -> zbus::Result<zbus_xml::Node<'static>> {
    let introspection = IntrospectableProxy::builder(connection)
        .destination(bus_name.to_owned())?
        .path(object_path.to_owned())?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let introspection_xml = introspection.introspect().await?;
    zbus_xml::Node::from_reader(introspection_xml.as_bytes())
        .map_err(|parse_error| zbus::Error::Failure(format!("introspection: {parse_error}")))
}
