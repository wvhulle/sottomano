use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use futures::{Stream, StreamExt, stream};
use serde::{Deserialize, Serialize};
use zbus::{
    names::OwnedBusName,
    proxy,
    proxy::CacheProperties,
    zvariant::{self, OwnedObjectPath, OwnedValue, Structure, Type, Value},
};

use crate::delivery_cadence_drivers::ChangeTriggerStreams;

pub mod gtk_actions_proxy;

use self::gtk_actions_proxy::{GtkActionDescription, gtk_actions_proxy};
use crate::desktop_windows_apps::{
    application_menu_nodes::{MenuKind, MenuNode, label_without_mnemonics},
    dbusmenu_menu_bar_layout::introspected_node,
};

const MENU_BAR_SUFFIX: &str = "/menus/menubar";
const WINDOW_ACTIONS_SUFFIX: &str = "/window";
const ROOT_MENU_GROUP: u32 = 0;
const SECTION_LINK: &str = ":section";
const SUBMENU_LINK: &str = ":submenu";

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

struct ScopedActionName {
    scope: GtkActionScope,
    action_name: String,
}

impl TryFrom<&str> for ScopedActionName {
    type Error = String;

    fn try_from(detailed_action: &str) -> Result<Self, Self::Error> {
        let Some((prefix, action_name)) = detailed_action.split_once('.') else {
            return Err(format!("{detailed_action} has no action group prefix"));
        };
        let scope = match prefix {
            "app" => GtkActionScope::Application,
            "win" => GtkActionScope::Window,
            _ => {
                return Err(format!(
                    "{detailed_action} belongs to an unexported action group"
                ));
            }
        };
        Ok(Self {
            scope,
            action_name: action_name.to_owned(),
        })
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct GtkMenuKey {
    group_id: u32,
    menu_id: u32,
}

impl TryFrom<&OwnedValue> for GtkMenuKey {
    type Error = zvariant::Error;

    fn try_from(link_value: &OwnedValue) -> Result<Self, Self::Error> {
        let link_structure = link_value.downcast_ref::<Structure<'_>>()?;
        match link_structure.fields() {
            [Value::U32(group_id), Value::U32(menu_id)] => Ok(Self {
                group_id: *group_id,
                menu_id: *menu_id,
            }),
            _ => Err(zvariant::Error::IncorrectType),
        }
    }
}

type GtkMenuItems = HashMap<GtkMenuKey, Vec<HashMap<String, OwnedValue>>>;
type GtkActionDescriptions = HashMap<String, GtkActionDescription>;

#[must_use]
pub fn application_object_path(app_id: &str) -> String {
    format!("/{}", app_id.replace('.', "/").replace('-', "_"))
}

async fn loaded_menu_items(menus_proxy: &GtkMenusProxy<'_>) -> zbus::Result<GtkMenuItems> {
    let mut menu_items = GtkMenuItems::new();
    let mut requested_groups = BTreeSet::new();
    let mut pending_groups = vec![ROOT_MENU_GROUP];
    while !pending_groups.is_empty() {
        requested_groups.extend(pending_groups.iter().copied());
        for group_reply in menus_proxy.start(&pending_groups).await? {
            menu_items.insert(
                GtkMenuKey {
                    group_id: group_reply.group_id,
                    menu_id: group_reply.menu_id,
                },
                group_reply.items,
            );
        }
        pending_groups = menu_items
            .values()
            .flatten()
            .filter_map(linked_menu)
            .map(|linked_key| linked_key.group_id)
            .filter(|group_id| !requested_groups.contains(group_id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    let started_groups: Vec<u32> = requested_groups.into_iter().collect();
    if let Err(end_failure) = menus_proxy.end(&started_groups).await {
        tracing::debug!("gtk menus: end subscription: {end_failure}");
    }
    Ok(menu_items)
}

fn linked_menu(menu_item: &HashMap<String, OwnedValue>) -> Option<GtkMenuKey> {
    [SECTION_LINK, SUBMENU_LINK]
        .into_iter()
        .find_map(|link_key| menu_item.get(link_key))
        .and_then(|link_value| GtkMenuKey::try_from(link_value).ok())
}

fn text_property(menu_item: &HashMap<String, OwnedValue>, property_name: &str) -> Option<String> {
    menu_item
        .get(property_name)
        .and_then(|property_value| property_value.downcast_ref::<&str>().ok())
        .map(str::to_owned)
}

struct GtkMenuTreeBuilder<'menus> {
    menu_items: &'menus GtkMenuItems,
    application_actions: &'menus GtkActionDescriptions,
    window_actions: &'menus GtkActionDescriptions,
    next_node_id: i32,
    activations: HashMap<i32, GtkMenuActivation>,
}

impl GtkMenuTreeBuilder<'_> {
    fn issue_node_id(&mut self) -> i32 {
        self.next_node_id += 1;
        self.next_node_id
    }

    fn menu_nodes(&mut self, menu_key: GtkMenuKey, depth_left: u8) -> Vec<MenuNode> {
        let Some(items) = self.menu_items.get(&menu_key) else {
            return Vec::new();
        };
        let mut menu_nodes = Vec::new();
        for menu_item in items {
            if let Some(section_key) = menu_item
                .get(SECTION_LINK)
                .and_then(|link_value| GtkMenuKey::try_from(link_value).ok())
            {
                let section_nodes = self.menu_nodes(section_key, depth_left.saturating_sub(1));
                if !menu_nodes.is_empty() && !section_nodes.is_empty() {
                    menu_nodes.push(self.separator_node());
                }
                menu_nodes.extend(section_nodes);
                continue;
            }
            menu_nodes.extend(self.item_node(menu_item, depth_left));
        }
        menu_nodes
    }

    fn separator_node(&mut self) -> MenuNode {
        MenuNode {
            id: self.issue_node_id(),
            label: None,
            enabled: true,
            kind: MenuKind::Separator,
            icon: None,
            children: Vec::new(),
        }
    }

    fn item_node(
        &mut self,
        menu_item: &HashMap<String, OwnedValue>,
        depth_left: u8,
    ) -> Option<MenuNode> {
        let label =
            text_property(menu_item, "label").map(|raw_label| label_without_mnemonics(&raw_label));
        let node_id = self.issue_node_id();
        if let Some(submenu_key) = menu_item
            .get(SUBMENU_LINK)
            .and_then(|link_value| GtkMenuKey::try_from(link_value).ok())
        {
            let children = if depth_left == 0 {
                Vec::new()
            } else {
                self.menu_nodes(submenu_key, depth_left - 1)
            };
            return Some(MenuNode {
                id: node_id,
                label,
                enabled: true,
                kind: MenuKind::Submenu,
                icon: None,
                children,
            });
        }
        let detailed_action = text_property(menu_item, "action")?;
        let ScopedActionName { scope, action_name } =
            ScopedActionName::try_from(detailed_action.as_str()).ok()?;
        let action_target = menu_item
            .get("target")
            .and_then(|target_value| target_value.try_clone().ok())
            .map_or(GtkActionTarget::Untargeted, |target_value| {
                GtkActionTarget::Targeted(Arc::new(target_value))
            });
        let description = match scope {
            GtkActionScope::Application => self.application_actions.get(&action_name),
            GtkActionScope::Window => self.window_actions.get(&action_name),
        };
        let enabled = description.is_some_and(|description| description.enabled);
        let kind = description
            .and_then(|description| description.state.first())
            .map_or(MenuKind::Standard, |action_state| {
                toggle_kind(action_state, &action_target)
            });
        self.activations.insert(
            node_id,
            GtkMenuActivation {
                scope,
                action_name,
                action_target,
            },
        );
        Some(MenuNode {
            id: node_id,
            label,
            enabled,
            kind,
            icon: None,
            children: Vec::new(),
        })
    }
}

fn toggle_kind(action_state: &OwnedValue, action_target: &GtkActionTarget) -> MenuKind {
    let action_state: &Value<'_> = action_state;
    let action_state = match action_state {
        Value::Value(boxed_state) => boxed_state.as_ref(),
        unboxed_state => unboxed_state,
    };
    match (action_state, action_target) {
        (Value::Bool(checked), GtkActionTarget::Untargeted) => MenuKind::Toggle(*checked),
        (current_state, GtkActionTarget::Targeted(target_value)) => {
            let target_value: &Value<'_> = target_value;
            MenuKind::Toggle(current_state == target_value)
        }
        (_, GtkActionTarget::Untargeted) => MenuKind::Standard,
    }
}

const MAXIMUM_MENU_DEPTH: u8 = 8;

fn gtk_menu_bar(
    menu_items: &GtkMenuItems,
    application_actions: &GtkActionDescriptions,
    window_actions: &GtkActionDescriptions,
    action_groups: GtkActionGroups,
) -> GtkMenuBar {
    let mut tree_builder = GtkMenuTreeBuilder {
        menu_items,
        application_actions,
        window_actions,
        next_node_id: 0,
        activations: HashMap::new(),
    };
    let top_menus = tree_builder.menu_nodes(
        GtkMenuKey {
            group_id: ROOT_MENU_GROUP,
            menu_id: 0,
        },
        MAXIMUM_MENU_DEPTH,
    );
    GtkMenuBar {
        top_menus,
        activations: tree_builder.activations,
        action_groups,
    }
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
    let Ok(windows_node) = introspected_node(connection, &bus_name.as_ref(), &windows_path).await
    else {
        return WindowActionPath::Unresolved;
    };
    match windows_node.nodes() {
        [single_window] => single_window
            .name()
            .and_then(|window_name| {
                OwnedObjectPath::try_from(format!("{windows_path}/{window_name}")).ok()
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

#[cfg(test)]
mod tests {
    use zbus::zvariant::{Signature, StructureBuilder};

    use super::*;

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
        let application_actions =
            GtkActionDescriptions::from([("quit".to_owned(), enabled_action())]);
        let menu_bar = gtk_menu_bar(
            &file_menu_bar(),
            &application_actions,
            &GtkActionDescriptions::new(),
            action_groups(),
        );
        let [file_menu] = menu_bar.top_menus.as_slice() else {
            panic!("one top menu expected, got {:?}", menu_bar.top_menus);
        };
        assert_eq!(file_menu.label.as_deref(), Some("File"));
        let child_kinds: Vec<MenuKind> =
            file_menu.children.iter().map(|child| child.kind).collect();
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
            .filter_map(|child| {
                child
                    .label
                    .as_deref()
                    .map(|label| format!("{label} enabled: {}", child.enabled))
            })
            .collect();
        assert_eq!(enabled_labels, ["New enabled: false", "Quit enabled: true"]);
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
}
