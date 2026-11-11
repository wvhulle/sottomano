use std::{collections::HashMap, future::ready, path::Path, time::Duration};

use futures::{Stream, StreamExt, stream::select};
use zbus::{
    Connection,
    fdo::{ManagedObjects, ObjectManagerProxy},
    names::OwnedInterfaceName,
    proxy,
    zvariant::OwnedObjectPath,
};

pub use crate::network_connectivity::hid_sysfs_bluetooth_peripherals::{
    ChargingState, DeviceBattery,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::{
        hid_sysfs_bluetooth_peripherals::hid_peripherals_with_address, interface,
        properties_changed,
    },
    stream_operators::{StreamOperators, resubscribed},
    system_protocol_clients::{
        dbus_desktop_buses::{DesktopBuses, Properties, name_owned, prop},
        sysfs_attribute_reads::read_trimmed,
    },
};

const SERVICE: &str = "org.bluez";
const ROOT: &str = "/";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const BATTERY: &str = "org.bluez.Battery1";
const BLUETOOTH_CLASS_DIRECTORY: &str = "/sys/class/bluetooth";
const PERIPHERAL_BATTERY_RECHECK: Duration = Duration::from_secs(60);

#[proxy(
    interface = "org.bluez.Adapter1",
    default_service = "org.bluez",
    gen_blocking = false
)]
pub trait BluezAdapter {
    #[zbus(property)]
    fn powered(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_powered(&self, powered: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.bluez.Device1",
    default_service = "org.bluez",
    gen_blocking = false
)]
pub trait BluezDevice {
    fn disconnect(&self) -> zbus::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Gamepad,
    Headset,
    Headphones,
    Speaker,
    Keyboard,
    Mouse,
    Tablet,
    Phone,
    Computer,
    Unclassified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedDevice {
    pub object_path: OwnedObjectPath,
    pub name: String,
    pub address: String,
    pub kind: DeviceKind,
    pub battery: DeviceBattery,
    pub kernel_devices: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoweredAdapter {
    pub connected: Vec<ConnectedDevice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterPower {
    Off,
    SwitchingOn,
    SwitchingOff,
    On(PoweredAdapter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothAdapter {
    pub object_path: OwnedObjectPath,
    pub controller_name: String,
    pub power: AdapterPower,
    pub placement: AdapterPlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterPlacement {
    BuiltIn,
    PluggedIn,
    Unknown,
}

impl AdapterPlacement {
    #[must_use]
    pub fn label(self) -> Option<&'static str> {
        match self {
            AdapterPlacement::BuiltIn => Some("built-in"),
            AdapterPlacement::PluggedIn => Some("USB"),
            AdapterPlacement::Unknown => None,
        }
    }
}

impl From<&str> for AdapterPlacement {
    fn from(controller_name: &str) -> Self {
        let device = Path::new(BLUETOOTH_CLASS_DIRECTORY)
            .join(controller_name)
            .join("device");
        match read_trimmed(device.join("../removable")).as_deref() {
            Some("fixed") => AdapterPlacement::BuiltIn,
            Some("removable") => AdapterPlacement::PluggedIn,
            _ if device.join("vendor").exists() => AdapterPlacement::BuiltIn,
            _ => AdapterPlacement::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bluetooth {
    pub adapters: Vec<BluetoothAdapter>,
}

impl From<&str> for DeviceKind {
    fn from(icon_name: &str) -> Self {
        match icon_name {
            "input-gaming" => DeviceKind::Gamepad,
            "audio-headset" => DeviceKind::Headset,
            "audio-headphones" => DeviceKind::Headphones,
            "audio-card" | "multimedia-player" => DeviceKind::Speaker,
            "input-keyboard" => DeviceKind::Keyboard,
            "input-mouse" => DeviceKind::Mouse,
            "input-tablet" => DeviceKind::Tablet,
            "phone" => DeviceKind::Phone,
            "computer" => DeviceKind::Computer,
            _ => DeviceKind::Unclassified,
        }
    }
}

impl AdapterPower {
    #[must_use]
    pub fn connected_devices(&self) -> &[ConnectedDevice] {
        match self {
            AdapterPower::On(powered) => &powered.connected,
            AdapterPower::Off | AdapterPower::SwitchingOn | AdapterPower::SwitchingOff => &[],
        }
    }
}

impl Bluetooth {
    pub fn connected_devices(&self) -> impl Iterator<Item = &ConnectedDevice> {
        self.adapters
            .iter()
            .flat_map(|adapter| adapter.power.connected_devices())
    }
}

type ObjectInterfaces = HashMap<OwnedInterfaceName, Properties>;

struct AdapterDeviceEntry<'objects> {
    object_path: &'objects OwnedObjectPath,
    interfaces: &'objects ObjectInterfaces,
    device: &'objects Properties,
}

fn bluez_battery(interfaces: &ObjectInterfaces) -> DeviceBattery {
    let Some(level_percent) =
        interface(interfaces, BATTERY).and_then(|battery| prop::<u8>(battery, "Percentage"))
    else {
        return DeviceBattery::Unreported;
    };
    DeviceBattery::Reported {
        level_percent,
        charging: ChargingState::Unknown,
    }
}

fn connected_device(entry: &AdapterDeviceEntry<'_>) -> Option<ConnectedDevice> {
    let device = entry.device;
    let address = prop::<String>(device, "Address")?;
    let name = prop::<String>(device, "Alias")
        .or_else(|| prop::<String>(device, "Name"))
        .unwrap_or_else(|| address.clone());
    let hid_peripherals = hid_peripherals_with_address(&address);
    let battery = bluez_battery(entry.interfaces).or_else(|| {
        hid_peripherals
            .iter()
            .map(|peripheral| peripheral.battery)
            .find(|battery| matches!(battery, DeviceBattery::Reported { .. }))
            .unwrap_or(DeviceBattery::Unreported)
    });
    Some(ConnectedDevice {
        object_path: entry.object_path.clone(),
        name,
        kind: prop::<String>(device, "Icon")
            .as_deref()
            .map_or(DeviceKind::Unclassified, DeviceKind::from),
        address,
        battery,
        kernel_devices: hid_peripherals
            .into_iter()
            .map(|peripheral| peripheral.kernel_device)
            .collect(),
    })
}

fn powered_adapter(objects: &ManagedObjects, adapter_path: &OwnedObjectPath) -> PoweredAdapter {
    let mut connected: Vec<ConnectedDevice> = objects
        .iter()
        .filter_map(|(object_path, interfaces)| {
            let device = interface(interfaces, DEVICE)?;
            let owned_by_adapter = prop::<OwnedObjectPath>(device, "Adapter")
                .is_some_and(|owner| owner == *adapter_path);
            owned_by_adapter.then_some(AdapterDeviceEntry {
                object_path,
                interfaces,
                device,
            })
        })
        .filter(|entry| prop::<bool>(entry.device, "Connected").unwrap_or_default())
        .filter_map(|entry| connected_device(&entry))
        .collect();
    connected.sort_by(|left, right| left.name.cmp(&right.name));
    PoweredAdapter { connected }
}

fn adapter_power(
    objects: &ManagedObjects,
    adapter_path: &OwnedObjectPath,
    adapter: &Properties,
) -> AdapterPower {
    match prop::<String>(adapter, "PowerState").as_deref() {
        Some("off-enabling") => AdapterPower::SwitchingOn,
        Some("on-disabling") => AdapterPower::SwitchingOff,
        _ if prop::<bool>(adapter, "Powered").unwrap_or(false) => {
            AdapterPower::On(powered_adapter(objects, adapter_path))
        }
        _ => AdapterPower::Off,
    }
}

fn reading(objects: &ManagedObjects) -> Bluetooth {
    let mut adapters: Vec<BluetoothAdapter> = objects
        .iter()
        .filter_map(|(adapter_path, interfaces)| {
            let adapter = interface(interfaces, ADAPTER)?;
            let controller_name = adapter_path.as_str().rsplit('/').next()?.to_owned();
            Some(BluetoothAdapter {
                object_path: adapter_path.clone(),
                power: adapter_power(objects, adapter_path, adapter),
                placement: AdapterPlacement::from(controller_name.as_str()),
                controller_name,
            })
        })
        .collect();
    adapters.sort_by(|left, right| left.controller_name.cmp(&right.controller_name));
    Bluetooth { adapters }
}

pub async fn read_bluetooth(connection: Connection) -> zbus::Result<Bluetooth> {
    let objects = ObjectManagerProxy::new(&connection, SERVICE, ROOT)
        .await?
        .get_managed_objects()
        .await?;
    Ok(reading(&objects))
}

async fn bluez_object_signals(connection: Connection) -> zbus::Result<impl Stream<Item = ()>> {
    let objects = ObjectManagerProxy::new(&connection, SERVICE, ROOT).await?;
    Ok(select(
        select(
            objects.receive_interfaces_added().await?.map(drop),
            objects.receive_interfaces_removed().await?.map(drop),
        ),
        properties_changed(&connection, SERVICE).await?.map(drop),
    ))
}

impl SnapshotSource for Bluetooth {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Bluetooth";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        name_owned(system.clone(), SERVICE).available_while(move || {
            let signal_connection = system.clone();
            let read_connection = system.clone();
            PeriodicPolling::every(PERIPHERAL_BATTERY_RECHECK).triggered_deliveries(
                resubscribed(Duration::from_secs(1), move || {
                    let signal_connection = signal_connection.clone();
                    async move {
                        Ok(bluez_object_signals(signal_connection)
                            .await?
                            .map(Ok::<_, zbus::Error>))
                    }
                })
                .filter_map(|signal| ready(signal.ok())),
                move || read_bluetooth(read_connection.clone()),
            )
        })
    }
}
