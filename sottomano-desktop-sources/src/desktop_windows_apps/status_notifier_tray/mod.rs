use std::{
    collections::HashMap,
    future::ready,
    sync::{Arc, PoisonError},
};

use futures::{Stream, StreamExt, TryStreamExt, stream};
use image::RgbaImage;
pub use system_tray::item::Status;
use system_tray::{
    client::{Client, Event, UpdateEvent},
    error::Error as TrayHostError,
    item::StatusNotifierItem,
    menu::TrayMenu,
};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use zbus::{fdo::DBusProxy, names::BusName};

use crate::{
    SnapshotDelivery, SnapshotSource,
    desktop_windows_apps::application_menu_nodes::{MenuNode, tray_menu_nodes},
    non_empty_text::non_empty,
    system_protocol_clients::dbus_desktop_buses::OwnerProcess,
    theme_icons::icon_requests::ItemThemePath,
};

mod tray_item_pixmaps;

pub(crate) use tray_item_pixmaps::capped_to_side_limit;
use tray_item_pixmaps::tray_item_icon;

const OWNER_LOOKUPS_IN_FLIGHT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayTooltip {
    NoTooltip,
    TitleOnly(String),
    BodyOnly(String),
    TitledBody { title: String, body: String },
}

fn tray_tooltip(title: String, description: String) -> TrayTooltip {
    let Some(title) = non_empty(title) else {
        return non_empty(description).map_or(TrayTooltip::NoTooltip, TrayTooltip::BodyOnly);
    };
    match non_empty(description) {
        Some(body) => TrayTooltip::TitledBody { title, body },
        None => TrayTooltip::TitleOnly(title),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayMenuPath {
    NotExported,
    Exported(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrayItemIcon {
    NoIcon,
    Pixmap(Arc<RgbaImage>),
    Named {
        icon_name: String,
        item_theme_path: ItemThemePath,
    },
}

#[derive(Debug, Clone)]
pub struct TrayItem {
    pub address: String,
    pub owner: OwnerProcess,
    pub id: String,
    pub name: String,
    pub status: Status,
    pub tooltip: TrayTooltip,
    pub menu_path: TrayMenuPath,
    pub icon: TrayItemIcon,
    pub menu: Vec<MenuNode>,
}

impl TrayItem {
    pub fn pixmaps(&self) -> impl Iterator<Item = &Arc<RgbaImage>> {
        let own_pixmap = match &self.icon {
            TrayItemIcon::Pixmap(pixels) => Some(pixels),
            TrayItemIcon::NoIcon | TrayItemIcon::Named { .. } => None,
        };
        own_pixmap
            .into_iter()
            .chain(self.menu.iter().flat_map(MenuNode::pixmaps))
    }
}

#[derive(Debug, Clone)]
pub enum TrayUpdate {
    Connected(TrayHost),
    Items(Vec<TrayItem>),
    HostFailed(Arc<TrayHostError>),
}

#[derive(Debug, Clone)]
pub struct TrayHost {
    pub tray_client: Arc<Client>,
    pub session_bus: zbus::Connection,
}

impl TrayHost {}

fn adapt_tooltip(item: &StatusNotifierItem) -> TrayTooltip {
    match &item.tool_tip {
        Some(tip) => tray_tooltip(tip.title.clone(), tip.description.clone()),
        None => TrayTooltip::NoTooltip,
    }
}

fn item_name(id: &str, title: Option<String>) -> String {
    title.and_then(non_empty).unwrap_or_else(|| id.to_owned())
}

fn exported_menu_path(menu_path: Option<String>) -> TrayMenuPath {
    menu_path.map_or(TrayMenuPath::NotExported, TrayMenuPath::Exported)
}

fn build_item(
    address: &str,
    owner: OwnerProcess,
    item: &StatusNotifierItem,
    menu: Option<&TrayMenu>,
) -> TrayItem {
    TrayItem {
        address: address.to_string(),
        owner,
        id: item.id.clone(),
        name: item_name(&item.id, item.title.clone()),
        status: item.status,
        tooltip: adapt_tooltip(item),
        menu_path: exported_menu_path(item.menu.clone()),
        icon: tray_item_icon(
            item.icon_name.as_deref(),
            item.icon_pixmap.as_deref(),
            item.icon_theme_path.as_deref(),
        ),
        menu: menu.map(tray_menu_nodes).unwrap_or_default(),
    }
}

#[derive(Clone)]
struct TrayOwnerLookup {
    dbus_proxy: DBusProxy<'static>,
}

impl TrayOwnerLookup {
    async fn connect() -> zbus::Result<Self> {
        let session_bus = zbus::Connection::session().await?;
        Ok(Self {
            dbus_proxy: DBusProxy::new(&session_bus).await?,
        })
    }

    fn session_bus(&self) -> zbus::Connection {
        self.dbus_proxy.inner().connection().clone()
    }

    async fn address_owner_process(&self, address: &str) -> OwnerProcess {
        match BusName::try_from(address) {
            Ok(bus_name) => OwnerProcess::query_bus_owner(&self.dbus_proxy, bus_name).await,
            Err(_) => OwnerProcess::Unidentified,
        }
    }

    async fn owners_of_listed(&self, client: &Client) -> HashMap<String, OwnerProcess> {
        let listed_items = client.items();
        let listed_addresses: Vec<String> = listed_items
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .cloned()
            .collect();
        let mut listed_owners = HashMap::new();
        for address in listed_addresses {
            let owner = self.address_owner_process(&address).await;
            listed_owners.insert(address, owner);
        }
        listed_owners
    }
}

struct ConnectedTrayHost {
    client: Arc<Client>,
    owner_lookup: TrayOwnerLookup,
    item_events: BroadcastStream<Event>,
    listed: Vec<TrayItem>,
}

async fn connect_tray_host() -> Result<ConnectedTrayHost, TrayHostError> {
    let client = Arc::new(Client::new().await?);
    let item_events = BroadcastStream::new(client.subscribe());
    let owner_lookup = TrayOwnerLookup::connect().await?;
    let listed_owners = owner_lookup.owners_of_listed(&client).await;
    let listed = listed_items(&client, &|address| {
        listed_owners
            .get(address)
            .copied()
            .unwrap_or(OwnerProcess::Unidentified)
    });
    Ok(ConnectedTrayHost {
        client,
        owner_lookup,
        item_events,
        listed,
    })
}

pub fn updates() -> impl Stream<Item = TrayUpdate> + Send {
    stream::once(connect_tray_host())
        .map_ok(|connected_host| hosted_updates(connected_host).map(Ok))
        .try_flatten()
        .map(|update| {
            update.unwrap_or_else(|host_failure| TrayUpdate::HostFailed(Arc::new(host_failure)))
        })
}

fn hosted_updates(connected_host: ConnectedTrayHost) -> impl Stream<Item = TrayUpdate> + Send {
    let ConnectedTrayHost {
        client,
        owner_lookup,
        item_events,
        listed,
    } = connected_host;
    let host = TrayHost {
        tray_client: client.clone(),
        session_bus: owner_lookup.session_bus(),
    };
    let item_changes = item_events
        .map(move |received| {
            let owner_lookup = owner_lookup.clone();
            async move {
                let added_owner = match &received {
                    Ok(Event::Add(address, _)) => owner_lookup.address_owner_process(address).await,
                    _ => OwnerProcess::Unidentified,
                };
                (received, added_owner)
            }
        })
        .buffered(OWNER_LOOKUPS_IN_FLIGHT)
        .scan(listed.clone(), move |items, (received, added_owner)| {
            match received {
                Ok(event) => apply(items, event, added_owner, &client),
                Err(BroadcastStreamRecvError::Lagged(_)) => {
                    let known_items = items.clone();
                    *items = listed_items(&client, &|address| {
                        known_items
                            .iter()
                            .find(|known_item| known_item.address == address)
                            .map_or(OwnerProcess::Unidentified, |known_item| known_item.owner)
                    });
                }
            }
            ready(Some(TrayUpdate::Items(items.clone())))
        });
    stream::iter([TrayUpdate::Connected(host), TrayUpdate::Items(listed)]).chain(item_changes)
}

impl SnapshotSource for Vec<TrayItem> {
    type Error = Arc<TrayHostError>;
    type StreamInputs = ();

    const NAME: &'static str = "Tray";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        updates()
            .filter_map(|update| {
                ready(match update {
                    TrayUpdate::Items(items) => Some(Ok(items)),
                    TrayUpdate::HostFailed(failure) => Some(Err(failure)),
                    TrayUpdate::Connected(_) => None,
                })
            })
            .map(SnapshotDelivery::live_push)
    }
}

fn listed_items(client: &Client, known_owner: &dyn Fn(&str) -> OwnerProcess) -> Vec<TrayItem> {
    client.items().lock().map_or_else(
        |_| Vec::new(),
        |map| {
            map.iter()
                .map(|(address, (item, menu))| {
                    build_item(address, known_owner(address), item, menu.as_ref())
                })
                .collect()
        },
    )
}

fn find_mut<'items>(items: &'items mut [TrayItem], address: &str) -> Option<&'items mut TrayItem> {
    items.iter_mut().find(|item| item.address == address)
}

fn menu_from_map(client: &Client, address: &str) -> Vec<MenuNode> {
    client
        .items()
        .lock()
        .inspect_err(|lock_failure| {
            tracing::warn!(
                "tray: item map lock poisoned reading menu of {address}: {lock_failure}"
            );
        })
        .ok()
        .and_then(|map| {
            map.get(address)
                .and_then(|(_, menu)| menu.as_ref())
                .map(tray_menu_nodes)
        })
        .unwrap_or_default()
}

fn apply(items: &mut Vec<TrayItem>, event: Event, added_owner: OwnerProcess, client: &Client) {
    match event {
        Event::Add(address, item) => {
            let built = build_item(&address, added_owner, &item, None);
            match find_mut(items, &address) {
                Some(existing) => *existing = built,
                None => items.push(built),
            }
        }
        Event::Remove(address) => items.retain(|item| item.address != address),
        Event::Update(address, update) => {
            let menu = matches!(update, UpdateEvent::Menu(_) | UpdateEvent::MenuDiff(_))
                .then(|| menu_from_map(client, &address));
            let Some(item) = find_mut(items, &address) else {
                return;
            };
            match update {
                UpdateEvent::Icon {
                    icon_name,
                    icon_pixmap,
                } => {
                    item.icon = tray_item_icon(icon_name.as_deref(), icon_pixmap.as_deref(), None);
                }
                UpdateEvent::Status(status) => item.status = status,
                UpdateEvent::Title(title) => item.name = item_name(&item.id, title),
                UpdateEvent::Tooltip(tooltip) => {
                    item.tooltip = match tooltip {
                        Some(tip) => tray_tooltip(tip.title, tip.description),
                        None => TrayTooltip::NoTooltip,
                    };
                }
                UpdateEvent::Menu(_) | UpdateEvent::MenuDiff(_) => {
                    item.menu = menu.unwrap_or_default();
                }
                UpdateEvent::MenuConnect(path) => item.menu_path = TrayMenuPath::Exported(path),
                UpdateEvent::AttentionIcon(_) | UpdateEvent::OverlayIcon(_) => {}
            }
        }
    }
}
