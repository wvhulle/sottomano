pub mod gtk_actions_proxy;
#[cfg(test)]
mod gtk_menu_bar_tests;
mod gtk_menu_item_tree;

use std::{collections::HashMap, sync::Arc};

use futures::{Stream, StreamExt, stream};
use serde::{Deserialize, Serialize};
use zbus::{
    names::OwnedBusName,
    proxy,
    proxy::CacheProperties,
    zvariant::{OwnedObjectPath, OwnedValue, Type},
};

use self::{
    gtk_actions_proxy::gtk_actions_proxy,
    gtk_menu_item_tree::{GtkActionDescriptions, gtk_menu_bar, loaded_menu_items},
};
use crate::{
    delivery_cadence_drivers::ChangeTriggerStreams,
    desktop_windows_apps::{
        application_menu_nodes::MenuNode, dbusmenu_menu_bar_layout::introspected_node,
    },
};

const MENU_BAR_SUFFIX: &str = "/menus/menubar";
const WINDOW_ACTIONS_SUFFIX: &str = "/window";

#[derive(Debug, Deserialize, Serialize, Type)]
pub struct GtkMenuGroupReply {
    group_id: u32,
    menu_id: u32,
    items: Vec<HashMap<String, OwnedValue>>,
}

#[proxy(interface = "org.gtk.Menus", assume_defaults = false)]
pub trait GtkMenus {
    fn start(&self, groups: &[u32]) -> zbus::Result<Vec<GtkMenuGroupReply>>;

    fn end(&self, groups: &[u32]) -> zbus::Result<()>;

    #[zbus(signal)]
    fn changed(&self, changes: Vec<OwnedValue>) -> zbus::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WindowActionPath {
    Resolved(OwnedObjectPath),
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ApplicationActionPath {
    Exported(OwnedObjectPath),
    Unexported,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GtkActionGroups {
    pub bus_name: OwnedBusName,
    pub application_actions: ApplicationActionPath,
    pub window_actions: WindowActionPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GtkMenuBarSource {
    pub menubar_path: OwnedObjectPath,
    pub action_groups: GtkActionGroups,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GtkActionScope {
    Application,
    Window,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GtkActionTarget {
    Untargeted,
    Targeted(Arc<OwnedValue>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GtkMenuActivation {
    pub scope: GtkActionScope,
    pub action_name: String,
    pub action_target: GtkActionTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GtkMenuBar {
    pub top_menus: Vec<MenuNode>,
    pub activations: HashMap<i32, GtkMenuActivation>,
    pub action_groups: GtkActionGroups,
}

#[must_use]
pub fn application_object_path(app_id: &str) -> String {
    format!("/{}", app_id.replace('.', "/").replace('-', "_"))
}

async fn described_actions(
    connection: &zbus::Connection,
    bus_name: &OwnedBusName,
    object_path: &OwnedObjectPath,
) -> GtkActionDescriptions {
    let described = match gtk_actions_proxy(connection, bus_name, object_path).await {
        Ok(actions_proxy) => actions_proxy.describe_all().await,
        Err(proxy_failure) => Err(proxy_failure),
    };
    described.unwrap_or_else(|describe_failure| {
        tracing::debug!("gtk actions {object_path}: {describe_failure}");
        GtkActionDescriptions::new()
    })
}

async fn single_window_action_path(
    connection: &zbus::Connection,
    bus_name: &OwnedBusName,
    application_path: &str,
) -> WindowActionPath {
    let windows_path = format!("{application_path}{WINDOW_ACTIONS_SUFFIX}");
    let windows_node = match introspected_node(connection, &bus_name.as_ref(), &windows_path).await
    {
        Ok(windows_node) => windows_node,
        Err(introspect_failure) => {
            tracing::debug!("gtk menus: no window actions at {windows_path}: {introspect_failure}");
            return WindowActionPath::Unresolved;
        }
    };
    match windows_node.nodes() {
        [single_window] => single_window
            .name()
            .and_then(|window_name| {
                OwnedObjectPath::try_from(format!("{windows_path}/{window_name}"))
                    .inspect_err(|path_failure| {
                        tracing::warn!(
                            "gtk menus: invalid window {window_name} under {windows_path}: \
                             {path_failure}"
                        );
                    })
                    .ok()
            })
            .map_or(WindowActionPath::Unresolved, WindowActionPath::Resolved),
        _ => WindowActionPath::Unresolved,
    }
}

pub async fn gtk_menu_bar_source_of_app(
    connection: &zbus::Connection,
    app_id: &str,
) -> zbus::Result<GtkMenuBarSource> {
    let bus_name = OwnedBusName::try_from(app_id)?;
    let application_path = application_object_path(app_id);
    let menubar_path = OwnedObjectPath::try_from(format!("{application_path}{MENU_BAR_SUFFIX}"))?;
    let window_actions = single_window_action_path(connection, &bus_name, &application_path).await;
    Ok(GtkMenuBarSource {
        menubar_path,
        action_groups: GtkActionGroups {
            bus_name,
            application_actions: ApplicationActionPath::Exported(OwnedObjectPath::try_from(
                application_path,
            )?),
            window_actions,
        },
    })
}

async fn fetched_gtk_menu_bar(
    connection: &zbus::Connection,
    menus_proxy: &GtkMenusProxy<'_>,
    action_groups: &GtkActionGroups,
) -> zbus::Result<GtkMenuBar> {
    let menu_items = loaded_menu_items(menus_proxy).await?;
    let application_actions = match &action_groups.application_actions {
        ApplicationActionPath::Exported(application_path) => {
            described_actions(connection, &action_groups.bus_name, application_path).await
        }
        ApplicationActionPath::Unexported => GtkActionDescriptions::new(),
    };
    let window_actions = match &action_groups.window_actions {
        WindowActionPath::Resolved(window_path) => {
            described_actions(connection, &action_groups.bus_name, window_path).await
        }
        WindowActionPath::Unresolved => GtkActionDescriptions::new(),
    };
    Ok(gtk_menu_bar(
        &menu_items,
        &application_actions,
        &window_actions,
        action_groups.clone(),
    ))
}

fn exported_action_paths(action_groups: &GtkActionGroups) -> Vec<OwnedObjectPath> {
    let application_path = match &action_groups.application_actions {
        ApplicationActionPath::Exported(application_path) => vec![application_path.clone()],
        ApplicationActionPath::Unexported => Vec::new(),
    };
    let window_path = match &action_groups.window_actions {
        WindowActionPath::Resolved(window_path) => vec![window_path.clone()],
        WindowActionPath::Unresolved => Vec::new(),
    };
    application_path.into_iter().chain(window_path).collect()
}

pub async fn gtk_menu_bar_updates(
    connection: zbus::Connection,
    source: GtkMenuBarSource,
) -> zbus::Result<impl Stream<Item = zbus::Result<GtkMenuBar>> + Send> {
    let GtkMenuBarSource {
        menubar_path,
        action_groups,
    } = source;
    let menus_proxy = GtkMenusProxy::builder(&connection)
        .destination(action_groups.bus_name.clone())?
        .path(menubar_path)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let mut change_streams = vec![
        menus_proxy
            .receive_changed()
            .await?
            .map(|_| Ok::<(), zbus::Error>(()))
            .boxed(),
    ];
    for action_path in exported_action_paths(&action_groups) {
        let action_changes = gtk_actions_proxy(&connection, &action_groups.bus_name, &action_path)
            .await?
            .receive_changed()
            .await?;
        change_streams.push(action_changes.map(|_| Ok(())).boxed());
    }
    Ok(
        stream::select_all(change_streams).triggered_rereads(move || {
            let connection = connection.clone();
            let menus_proxy = menus_proxy.clone();
            let action_groups = action_groups.clone();
            async move { fetched_gtk_menu_bar(&connection, &menus_proxy, &action_groups).await }
        }),
    )
}
