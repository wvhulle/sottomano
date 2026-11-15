use sottomano_desktop_sources::network_connectivity::network_manager_proxies::{
    self, DeviceInfo, NmConnectionSwitch,
};
use zbus::{
    Connection,
    zvariant::{ObjectPath, OwnedObjectPath},
};

use crate::sink_write_errors::BusWriteError;

async fn activate(
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

async fn deactivate(
    connection: &Connection,
    active_connection: &OwnedObjectPath,
) -> zbus::Result<()> {
    network_manager_proxies::network_manager_proxy(connection)
        .await?
        .deactivate_connection(active_connection)
        .await
}

async fn toggle_wireless_radio(connection: &Connection) -> zbus::Result<()> {
    let manager = network_manager_proxies::network_manager_proxy(connection).await?;
    let enabled = manager.wireless_enabled().await?;
    manager.set_wireless_enabled(!enabled).await
}

async fn toggle_wwan_radio(connection: &Connection) -> zbus::Result<()> {
    let manager = network_manager_proxies::network_manager_proxy(connection).await?;
    let enabled = manager.wwan_enabled().await?;
    manager.set_wwan_enabled(!enabled).await
}

async fn disconnect_device(device: &DeviceInfo) -> zbus::Result<()> {
    device.proxy.disconnect().await
}

async fn connect_device(connection: &Connection, device: &DeviceInfo) -> zbus::Result<()> {
    let unset = ObjectPath::from_static_str_unchecked("/");
    network_manager_proxies::network_manager_proxy(connection)
        .await?
        .activate_connection(&unset.clone().into(), &device.path, &unset)
        .await
        .map(drop)
}

async fn toggle_device_connection(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<()> {
    match device.link_state {
        network_manager_proxies::LinkPhase::Up => disconnect_device(device).await,
        network_manager_proxies::LinkPhase::Switching
        | network_manager_proxies::LinkPhase::Down => connect_device(connection, device).await,
    }
}

pub async fn toggle_wifi_radio() -> Result<(), BusWriteError> {
    Ok(toggle_wireless_radio(&Connection::system().await?).await?)
}

pub async fn toggle_wifi_adapter(adapter_device: &str) -> Result<(), BusWriteError> {
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
    Ok(toggle_device_connection(&system, &wifi_device).await?)
}

pub async fn toggle_ethernet_port(port_name: &str) -> Result<(), BusWriteError> {
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
    Ok(toggle_device_connection(&system, &port_device).await?)
}

pub async fn toggle_mobile_radio() -> Result<(), BusWriteError> {
    Ok(toggle_wwan_radio(&Connection::system().await?).await?)
}

pub async fn toggle_mobile_modem(modem_path: &str) -> Result<(), BusWriteError> {
    let system = Connection::system().await?;
    let Some(modem_device) = network_manager_proxies::modem_device_at(&system, modem_path).await?
    else {
        tracing::warn!("no NetworkManager device manages the modem at {modem_path}");
        return Ok(());
    };
    Ok(toggle_device_connection(&system, &modem_device).await?)
}

pub async fn switch_nm_connection(
    connection_switch: NmConnectionSwitch,
) -> Result<(), BusWriteError> {
    let system = Connection::system().await?;
    match &connection_switch {
        NmConnectionSwitch::Connect(settings_connection) => {
            activate(&system, settings_connection).await?;
        }
        NmConnectionSwitch::Disconnect(active_connection) => {
            deactivate(&system, active_connection).await?;
        }
    }
    Ok(())
}
