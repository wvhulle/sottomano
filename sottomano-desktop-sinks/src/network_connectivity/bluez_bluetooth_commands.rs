use sottomano_desktop_sources::network_connectivity::bluez_bluetooth::{
    BluezAdapterProxy, BluezDeviceProxy, read_bluetooth,
};
use zbus::{Connection, zvariant::OwnedObjectPath};

pub async fn toggle_adapter_power(adapter: OwnedObjectPath) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let proxy = BluezAdapterProxy::builder(&connection)
        .path(adapter)?
        .build()
        .await?;
    let powered = proxy.powered().await?;
    proxy.set_powered(!powered).await
}

async fn disconnect_on(connection: &Connection, device: OwnedObjectPath) -> zbus::Result<()> {
    BluezDeviceProxy::builder(connection)
        .path(device)?
        .build()
        .await?
        .disconnect()
        .await
}

pub async fn disconnect_bluetooth_device(device: OwnedObjectPath) -> zbus::Result<()> {
    disconnect_on(&Connection::system().await?, device).await
}

pub async fn disconnect_bluetooth_address(hardware_address: String) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let bluetooth = read_bluetooth(connection.clone()).await?;
    let Some(device) = bluetooth
        .connected_devices()
        .find(|device| device.address.eq_ignore_ascii_case(&hardware_address))
    else {
        return Ok(());
    };
    disconnect_on(&connection, device.object_path.clone()).await
}
