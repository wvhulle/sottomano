use std::future::ready;

use futures::{Stream, StreamExt, stream};
use serde::Serialize;
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use zbus::{
    Connection, connection,
    fdo::{self, DBusProxy, RequestNameFlags, RequestNameReply},
    message::Header,
    names::{BusName, OwnedBusName, OwnedUniqueName},
    object_server::SignalEmitter,
    zvariant::{OwnedObjectPath, Type},
};

use crate::{
    desktop_windows_apps::dbusmenu_menu_bar_layout::DbusMenuLocation,
    system_protocol_clients::{
        dbus_desktop_buses::{OwnerProcess, served_while_connected},
        protocol_peer_reconnects::subscribed,
    },
};

const REGISTRAR_PATH: &str = "/com/canonical/AppMenu/Registrar";
const REGISTRAR_NAME: &str = "com.canonical.AppMenu.Registrar";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegisteredMenuBar {
    pub window_id: u32,
    pub owner_bus_name: OwnedUniqueName,
    pub owner_process: OwnerProcess,
    pub menu_path: OwnedObjectPath,
}

impl From<&RegisteredMenuBar> for DbusMenuLocation {
    fn from(registered: &RegisteredMenuBar) -> Self {
        Self {
            bus_name: OwnedBusName::from(BusName::Unique(registered.owner_bus_name.as_ref())),
            menu_path: registered.menu_path.clone(),
        }
    }
}

#[derive(Debug, Serialize, Type)]
struct RegisteredMenuLocation {
    bus_name: String,
    object_path: OwnedObjectPath,
}

#[derive(Debug, Serialize, Type)]
struct RegisteredWindowMenu {
    window_id: u32,
    bus_name: String,
    object_path: OwnedObjectPath,
}

struct AppMenuRegistrar {
    registered_menu_bars: watch::Sender<Vec<RegisteredMenuBar>>,
}

#[zbus::interface(name = "com.canonical.AppMenu.Registrar")]
impl AppMenuRegistrar {
    async fn register_window(
        &self,
        window_id: u32,
        menu_object_path: OwnedObjectPath,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        let Some(sender) = header.sender() else {
            return Err(fdo::Error::Failed("registration without a sender".into()));
        };
        let owner_bus_name = OwnedUniqueName::from(sender.clone());
        let dbus = DBusProxy::new(connection).await?;
        let owner_process = OwnerProcess::of_bus(&dbus, BusName::from(sender.clone())).await;
        tracing::debug!(
            "appmenu registrar: window {window_id} of {owner_bus_name} exports {menu_object_path}"
        );
        let registered_menu_bar = RegisteredMenuBar {
            window_id,
            owner_bus_name: owner_bus_name.clone(),
            owner_process,
            menu_path: menu_object_path.clone(),
        };
        self.registered_menu_bars
            .send_modify(|registered_menu_bars| {
                registered_menu_bars.retain(|registered| registered.window_id != window_id);
                registered_menu_bars.push(registered_menu_bar);
            });
        Self::window_registered(
            &emitter,
            window_id,
            owner_bus_name.as_str(),
            &menu_object_path,
        )
        .await?;
        Ok(())
    }

    async fn unregister_window(
        &self,
        window_id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<()> {
        self.registered_menu_bars
            .send_modify(|registered_menu_bars| {
                registered_menu_bars.retain(|registered| registered.window_id != window_id);
            });
        Self::window_unregistered(&emitter, window_id).await?;
        Ok(())
    }

    fn get_menu_for_window(&self, window_id: u32) -> fdo::Result<RegisteredMenuLocation> {
        self.registered_menu_bars
            .borrow()
            .iter()
            .find(|registered| registered.window_id == window_id)
            .map(|registered| RegisteredMenuLocation {
                bus_name: registered.owner_bus_name.to_string(),
                object_path: registered.menu_path.clone(),
            })
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("window {window_id} is not registered")))
    }

    fn get_menus(&self) -> Vec<RegisteredWindowMenu> {
        self.registered_menu_bars
            .borrow()
            .iter()
            .map(|registered| RegisteredWindowMenu {
                window_id: registered.window_id,
                bus_name: registered.owner_bus_name.to_string(),
                object_path: registered.menu_path.clone(),
            })
            .collect()
    }

    #[zbus(signal)]
    async fn window_registered(
        emitter: &SignalEmitter<'_>,
        window_id: u32,
        service: &str,
        menu_object_path: &OwnedObjectPath,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn window_unregistered(emitter: &SignalEmitter<'_>, window_id: u32) -> zbus::Result<()>;
}

pub fn registrations() -> impl Stream<Item = Vec<RegisteredMenuBar>> + Send {
    use futures::StreamExt;
    subscribed((), |()| async {
        Ok::<_, zbus::Error>(serve().await?.map(Ok::<_, zbus::Error>))
    })
    .filter_map(|delivery| {
        ready(
            delivery
                .inspect_err(|serve_failure| tracing::warn!("appmenu registrar: {serve_failure}"))
                .ok(),
        )
    })
}

async fn serve() -> zbus::Result<impl Stream<Item = Vec<RegisteredMenuBar>> + Send> {
    let (registered_menu_bars, registration_changes) = watch::channel(Vec::new());
    let connection = connection::Builder::session()?
        .serve_at(
            REGISTRAR_PATH,
            AppMenuRegistrar {
                registered_menu_bars: registered_menu_bars.clone(),
            },
        )?
        .build()
        .await?;
    claim_registrar_name(&connection).await?;
    let vanished_owners = DBusProxy::new(&connection)
        .await?
        .receive_name_owner_changed()
        .await?;
    let pruned_on_vanish = vanished_owners.filter_map(move |owner_change| {
        let vanished_name = owner_change
            .args()
            .inspect_err(|args_failure| {
                tracing::warn!("appmenu registrar: invalid owner change: {args_failure}");
            })
            .ok()
            .and_then(|change_args| match change_args.name() {
                BusName::Unique(unique_name) if change_args.new_owner().is_none() => {
                    Some(unique_name.to_owned())
                }
                _ => None,
            });
        if let Some(vanished_name) = vanished_name {
            registered_menu_bars.send_if_modified(|registered| {
                let before = registered.len();
                registered.retain(|menu_bar| *menu_bar.owner_bus_name != vanished_name);
                registered.len() != before
            });
        }
        ready(None)
    });
    Ok(served_while_connected(
        stream::select(WatchStream::new(registration_changes), pruned_on_vanish),
        connection,
    ))
}

async fn claim_registrar_name(connection: &Connection) -> zbus::Result<()> {
    let reply = connection
        .request_name_with_flags(REGISTRAR_NAME, RequestNameFlags::DoNotQueue.into())
        .await?;
    match reply {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => Ok(()),
        RequestNameReply::InQueue | RequestNameReply::Exists => Err(zbus::Error::Failure(format!(
            "{REGISTRAR_NAME} is held by another global menu service"
        ))),
    }
}
