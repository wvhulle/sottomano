use std::collections::HashMap;

use zbus::{Connection, fdo::ObjectManagerProxy};

use crate::{network_connectivity::interface, system_protocol_clients::dbus_desktop_buses::prop};

const BLUEZ_SERVICE: &str = "org.bluez";
const BLUEZ_ROOT: &str = "/";
const BLUEZ_DEVICE_INTERFACE: &str = "org.bluez.Device1";

async fn bluetooth_alias_lookup(connection: &Connection) -> zbus::Result<HashMap<String, String>> {
    let objects = ObjectManagerProxy::builder(connection)
        .destination(BLUEZ_SERVICE)?
        .path(BLUEZ_ROOT)?
        .build()
        .await?
        .get_managed_objects()
        .await?;
    Ok(objects
        .values()
        .filter_map(|interfaces| {
            let device = interface(interfaces, BLUEZ_DEVICE_INTERFACE)?;
            Some((
                prop::<String>(device, "Address")?.to_lowercase(),
                prop::<String>(device, "Alias")?,
            ))
        })
        .collect())
}

pub async fn bluetooth_aliases(connection: &Connection) -> HashMap<String, String> {
    bluetooth_alias_lookup(connection)
        .await
        .inspect_err(|e| tracing::debug!("gamepad: no Bluetooth aliases: {e}"))
        .unwrap_or_default()
}
