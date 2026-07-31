use iced::futures::{StreamExt, stream::BoxStream};
use zbus::{Connection, MatchRule, MessageStream, proxy, zvariant::OwnedObjectPath};

pub const SERVICE: &str = "org.freedesktop.NetworkManager";

pub const TYPE_ETHERNET: u32 = 1;
pub const TYPE_WIFI: u32 = 2;
pub const STATE_ACTIVATED: u32 = 100;

pub const DASH: &str = "—";

#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait Manager {
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Device {
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn interface(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn ip4_config(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn active_connection(&self) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Wireless {
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;
    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;
    #[zbus(property)]
    fn frequency(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn wpa_flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn rsn_flags(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait ActiveConnection {
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.IP4Config",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Ip4Config {
    #[zbus(property)]
    fn address_data(
        &self,
    ) -> zbus::Result<Vec<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>>;
    #[zbus(property)]
    fn gateway(&self) -> zbus::Result<String>;
}

pub struct DeviceInfo {
    pub interface: String,
    pub kind: u32,
    pub connected: bool,
    pub path: OwnedObjectPath,
}

pub async fn devices(connection: &Connection) -> Vec<DeviceInfo> {
    let Ok(manager) = ManagerProxy::new(connection).await else {
        return Vec::new();
    };
    let Ok(paths) = manager.get_devices().await else {
        return Vec::new();
    };
    let queries = paths.into_iter().map(|path| device_info(connection, path));
    iced::futures::future::join_all(queries)
        .await
        .into_iter()
        .flatten()
        .collect()
}

async fn device_info(connection: &Connection, path: OwnedObjectPath) -> Option<DeviceInfo> {
    let device = DeviceProxy::builder(connection)
        .path(path.clone())
        .ok()?
        .build()
        .await
        .ok()?;
    Some(DeviceInfo {
        kind: device.device_type().await.ok()?,
        interface: device.interface().await.ok()?,
        connected: device.state().await.ok()? == STATE_ACTIVATED,
        path,
    })
}

pub async fn connection_id(connection: &Connection, device: &OwnedObjectPath) -> String {
    let active = async {
        let device = DeviceProxy::builder(connection)
            .path(device.clone())?
            .build()
            .await?;
        let path = device.active_connection().await?;
        ActiveConnectionProxy::builder(connection)
            .path(path)?
            .build()
            .await?
            .id()
            .await
    }
    .await;
    non_empty(active.ok())
}

pub struct Ip {
    pub address: String,
    pub gateway: String,
}

pub async fn ip(connection: &Connection, device: &OwnedObjectPath) -> Ip {
    let config = async {
        let device = DeviceProxy::builder(connection)
            .path(device.clone())?
            .build()
            .await?;
        let path = device.ip4_config().await?;
        Ip4ConfigProxy::builder(connection)
            .path(path)?
            .build()
            .await
    }
    .await;
    let Ok(config) = config else {
        return Ip {
            address: DASH.to_owned(),
            gateway: DASH.to_owned(),
        };
    };
    let address = config.address_data().await.ok().and_then(|entries| {
        let first = entries.first()?;
        let ip = first.get("address")?.downcast_ref::<String>().ok()?;
        let prefix = first.get("prefix")?.downcast_ref::<u32>().ok()?;
        Some(format!("{ip}/{prefix}"))
    });
    Ip {
        address: address.unwrap_or_else(|| DASH.to_owned()),
        gateway: non_empty(config.gateway().await.ok()),
    }
}

pub struct ApInfo {
    pub ssid: String,
    pub signal: i64,
    pub frequency: String,
    pub security: String,
}

pub async fn access_point(connection: &Connection, device: &OwnedObjectPath) -> Option<ApInfo> {
    let wireless = WirelessProxy::builder(connection)
        .path(device.clone())
        .ok()?
        .build()
        .await
        .ok()?;
    let ap_path = wireless.active_access_point().await.ok()?;
    let ap = AccessPointProxy::builder(connection)
        .path(ap_path)
        .ok()?
        .build()
        .await
        .ok()?;
    let ssid = ap
        .ssid()
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    Some(ApInfo {
        ssid: dash(&ssid),
        signal: ap.strength().await.map_or(0, i64::from),
        frequency: ap
            .frequency()
            .await
            .map_or_else(|_| DASH.to_owned(), |mhz| format!("{mhz} MHz")),
        security: security(
            ap.flags().await.unwrap_or(0),
            ap.wpa_flags().await.unwrap_or(0),
            ap.rsn_flags().await.unwrap_or(0),
        ),
    })
}

const AP_PRIVACY: u32 = 0x1;
const KEY_MGMT_8021X: u32 = 0x200;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;

fn security(flags: u32, wpa: u32, rsn: u32) -> String {
    let parts = [
        (flags & AP_PRIVACY != 0 && wpa == 0 && rsn == 0).then_some("WEP"),
        (wpa != 0).then_some("WPA1"),
        (rsn & (KEY_MGMT_PSK | KEY_MGMT_8021X) != 0).then_some("WPA2"),
        (rsn & KEY_MGMT_SAE != 0).then_some("WPA3"),
        (rsn & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0).then_some("OWE"),
        (wpa & KEY_MGMT_8021X != 0 || rsn & (KEY_MGMT_8021X | KEY_MGMT_EAP_SUITE_B_192) != 0)
            .then_some("802.1X"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    dash(&parts)
}

fn dash(value: &str) -> String {
    if value.is_empty() || value == "--" {
        DASH.to_owned()
    } else {
        value.to_owned()
    }
}

fn non_empty(value: Option<String>) -> String {
    value
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DASH.to_owned())
}

pub async fn changes(connection: &Connection) -> zbus::Result<BoxStream<'static, ()>> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(SERVICE)?
        .build();
    Ok(MessageStream::for_match_rule(rule, connection, Some(16))
        .await?
        .map(|_| ())
        .boxed())
}
