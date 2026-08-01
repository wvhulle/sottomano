use iced::futures::{StreamExt, stream::BoxStream};
use zbus::{Connection, MatchRule, MessageStream, proxy, zvariant::OwnedObjectPath};

pub const SERVICE: &str = "org.freedesktop.NetworkManager";

pub const TYPE_ETHERNET: u32 = 1;
pub const TYPE_WIFI: u32 = 2;
pub const STATE_ACTIVATED: u32 = 100;

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
    fn ip6_config(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn active_connection(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn hw_address(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn mtu(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wired",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Wired {
    #[zbus(property)]
    fn speed(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Wireless {
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn bitrate(&self) -> zbus::Result<u32>;
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
    #[zbus(property)]
    fn nameserver_data(
        &self,
    ) -> zbus::Result<Vec<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.IP6Config",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Ip6Config {
    #[zbus(property)]
    fn address_data(
        &self,
    ) -> zbus::Result<Vec<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>>;
    #[zbus(property)]
    fn gateway(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn nameservers(&self) -> zbus::Result<Vec<Vec<u8>>>;
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

pub async fn connection_id(connection: &Connection, device: &OwnedObjectPath) -> Option<String> {
    async {
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
    .await
    .ok()
}

#[derive(Debug, Clone)]
pub struct AddressBlock {
    pub address: String,
    pub mask: String,
    pub gateway: Option<String>,
}

pub struct IpInfo {
    pub v4: Option<AddressBlock>,
    pub v6: Option<AddressBlock>,
    pub dns: Vec<String>,
}

pub async fn ip(connection: &Connection, device: &OwnedObjectPath) -> IpInfo {
    let paths = async {
        let device = DeviceProxy::builder(connection)
            .path(device.clone())?
            .build()
            .await?;
        zbus::Result::Ok((device.ip4_config().await?, device.ip6_config().await?))
    }
    .await;
    let Ok((v4_path, v6_path)) = paths else {
        return IpInfo {
            v4: None,
            v6: None,
            dns: Vec::new(),
        };
    };
    let mut dns = Vec::new();
    let v4 = ip4_block(connection, v4_path, &mut dns).await;
    let v6 = ip6_block(connection, v6_path, &mut dns).await;
    IpInfo { v4, v6, dns }
}

fn first_address(
    entries: &[std::collections::HashMap<String, zbus::zvariant::OwnedValue>],
) -> Option<(String, u32)> {
    let global = entries.iter().find(|entry| {
        entry
            .get("address")
            .and_then(|value| value.downcast_ref::<String>().ok())
            .is_some_and(|address| !address.starts_with("fe80"))
    });
    let entry = global.or_else(|| entries.first())?;
    let address = entry.get("address")?.downcast_ref::<String>().ok()?;
    let prefix = entry.get("prefix")?.downcast_ref::<u32>().ok()?;
    Some((address, prefix))
}

async fn ip4_block(
    connection: &Connection,
    path: OwnedObjectPath,
    dns: &mut Vec<String>,
) -> Option<AddressBlock> {
    let config = Ip4ConfigProxy::builder(connection)
        .path(path)
        .ok()?
        .build()
        .await
        .ok()?;
    if let Ok(servers) = config.nameserver_data().await {
        dns.extend(
            servers
                .into_iter()
                .filter_map(|entry| entry.get("address")?.downcast_ref::<String>().ok()),
        );
    }
    let (address, prefix) = first_address(&config.address_data().await.ok()?)?;
    Some(AddressBlock {
        address,
        mask: prefix_to_netmask(prefix),
        gateway: config
            .gateway()
            .await
            .ok()
            .filter(|gateway| !gateway.is_empty()),
    })
}

async fn ip6_block(
    connection: &Connection,
    path: OwnedObjectPath,
    dns: &mut Vec<String>,
) -> Option<AddressBlock> {
    let config = Ip6ConfigProxy::builder(connection)
        .path(path)
        .ok()?
        .build()
        .await
        .ok()?;
    if let Ok(servers) = config.nameservers().await {
        dns.extend(servers.into_iter().filter_map(|bytes| {
            let raw = <[u8; 16]>::try_from(bytes.as_slice()).ok()?;
            Some(std::net::Ipv6Addr::from(raw).to_string())
        }));
    }
    let (address, prefix) = first_address(&config.address_data().await.ok()?)?;
    Some(AddressBlock {
        address,
        mask: format!("/{prefix}"),
        gateway: config
            .gateway()
            .await
            .ok()
            .filter(|gateway| !gateway.is_empty()),
    })
}

fn prefix_to_netmask(prefix: u32) -> String {
    let bits = match prefix {
        0 => 0,
        32.. => !0,
        _ => !0u32 << (32 - prefix),
    };
    std::net::Ipv4Addr::from(bits).to_string()
}

pub async fn hw_address(connection: &Connection, device: &OwnedObjectPath) -> Option<String> {
    async {
        DeviceProxy::builder(connection)
            .path(device.clone())?
            .build()
            .await?
            .hw_address()
            .await
    }
    .await
    .ok()
}

pub async fn mtu(connection: &Connection, device: &OwnedObjectPath) -> u32 {
    async {
        DeviceProxy::builder(connection)
            .path(device.clone())?
            .build()
            .await?
            .mtu()
            .await
    }
    .await
    .unwrap_or(0)
}

pub async fn wired_speed(connection: &Connection, device: &OwnedObjectPath) -> u32 {
    async {
        WiredProxy::builder(connection)
            .path(device.clone())?
            .build()
            .await?
            .speed()
            .await
    }
    .await
    .unwrap_or(0)
}

pub async fn wireless_rate(connection: &Connection, device: &OwnedObjectPath) -> u32 {
    async {
        WirelessProxy::builder(connection)
            .path(device.clone())?
            .build()
            .await?
            .bitrate()
            .await
    }
    .await
    .unwrap_or(0)
}

pub struct ApInfo {
    pub ssid: Option<String>,
    pub signal: i64,
    pub frequency: Option<String>,
    pub security: Option<String>,
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
    Some(ApInfo {
        ssid: ap
            .ssid()
            .await
            .ok()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .filter(|ssid| !ssid.is_empty()),
        signal: ap.strength().await.map_or(0, i64::from),
        frequency: ap.frequency().await.ok().map(|mhz| format!("{mhz} MHz")),
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

fn security(flags: u32, wpa: u32, rsn: u32) -> Option<String> {
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
    (!parts.is_empty()).then_some(parts)
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
