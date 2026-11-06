use sottomano_desktop_sources::network_connectivity::{
    network_manager_proxies::{self, DeviceInfo},
    network_manager_vpn_connections::VpnSwitch,
    wireguard_tunnels::WireguardSwitch,
};
use zbus::{
    Connection,
    zvariant::{ObjectPath, OwnedObjectPath},
};

pub async fn activate(
    connection: &Connection,
    settings_connection: &OwnedObjectPath,
) -> zbus::Result<()> {
    let unset = ObjectPath::from_static_str_unchecked("/");
    network_manager_proxies::network_manager_proxy(connection)
        .await?
        .activate_connection(settings_connection, &unset, &unset)
        .await
        .map(drop)
}

pub async fn deactivate(
    connection: &Connection,
    active_connection: &OwnedObjectPath,
) -> zbus::Result<()> {
    network_manager_proxies::network_manager_proxy(connection)
        .await?
        .deactivate_connection(active_connection)
        .await
}

pub async fn toggle_wireless_radio(connection: &Connection) -> zbus::Result<()> {
    let manager = network_manager_proxies::network_manager_proxy(connection).await?;
    let enabled = manager.wireless_enabled().await?;
    manager.set_wireless_enabled(!enabled).await
}

pub async fn toggle_wwan_radio(connection: &Connection) -> zbus::Result<()> {
    let manager = network_manager_proxies::network_manager_proxy(connection).await?;
    let enabled = manager.wwan_enabled().await?;
    manager.set_wwan_enabled(!enabled).await
}

pub async fn disconnect_device(device: &DeviceInfo) -> zbus::Result<()> {
    device.proxy.disconnect().await
}

pub async fn connect_device(connection: &Connection, device: &DeviceInfo) -> zbus::Result<()> {
    let unset = ObjectPath::from_static_str_unchecked("/");
    network_manager_proxies::network_manager_proxy(connection)
        .await?
        .activate_connection(&unset.clone().into(), &device.path, &unset)
        .await
        .map(drop)
}

pub async fn toggle_device_connection(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<()> {
    if device.connected {
        disconnect_device(device).await
    } else {
        connect_device(connection, device).await
    }
}

pub async fn toggle_wifi_radio() -> zbus::Result<()> {
    toggle_wireless_radio(&Connection::system().await?).await
}

pub async fn toggle_wifi_adapter(adapter_device: &str) -> zbus::Result<()> {
    let system = Connection::system().await?;
    let Some(wifi_device) = network_manager_proxies::devices(&system)
        .await?
        .into_iter()
        .find(|device| {
            device.kind == network_manager_proxies::DeviceKind::Wifi
                && device.interface == adapter_device
        })
    else {
        return Ok(());
    };
    toggle_device_connection(&system, &wifi_device).await
}

pub async fn toggle_ethernet_port(port_name: &str) -> zbus::Result<()> {
    let system = Connection::system().await?;
    let Some(port_device) = network_manager_proxies::devices(&system)
        .await?
        .into_iter()
        .find(|device| {
            device.kind == network_manager_proxies::DeviceKind::Ethernet
                && device.interface == port_name
        })
    else {
        return Ok(());
    };
    toggle_device_connection(&system, &port_device).await
}

pub async fn toggle_mobile_radio() -> zbus::Result<()> {
    toggle_wwan_radio(&Connection::system().await?).await
}

pub async fn toggle_mobile_modem(modem_path: &str) -> zbus::Result<()> {
    let system = Connection::system().await?;
    let Some(modem_device) = network_manager_proxies::modem_device_at(&system, modem_path).await?
    else {
        tracing::warn!("no NetworkManager device manages the modem at {modem_path}");
        return Ok(());
    };
    toggle_device_connection(&system, &modem_device).await
}

pub async fn switch_vpn(vpn_switch: VpnSwitch) {
    let switched = match Connection::system().await {
        Ok(system) => match &vpn_switch {
            VpnSwitch::Connect(settings_connection) => activate(&system, settings_connection).await,
            VpnSwitch::Disconnect(active_connection) => {
                deactivate(&system, active_connection).await
            }
        },
        Err(bus_error) => Err(bus_error),
    };
    if let Err(bus_error) = switched {
        tracing::warn!("NetworkManager VPN {vpn_switch:?}: {bus_error}");
    }
}

pub async fn switch_wireguard(wireguard_switch: WireguardSwitch) {
    let switched = match Connection::system().await {
        Ok(system) => match &wireguard_switch {
            WireguardSwitch::Connect(settings_connection) => {
                activate(&system, settings_connection).await
            }
            WireguardSwitch::Disconnect(active_connection) => {
                deactivate(&system, active_connection).await
            }
        },
        Err(bus_error) => Err(bus_error),
    };
    if let Err(bus_error) = switched {
        tracing::warn!("WireGuard {wireguard_switch:?}: {bus_error}");
    }
}
