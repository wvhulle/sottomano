use futures::Stream;
use zbus::fdo::ManagedObjects;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    hardware::Adapter,
    net_interfaces::{device_or_absent, interface, managed_objects_readings},
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, prop},
};

pub const NAME: &str = "bluetooth";

const SERVICE: &str = "org.bluez";
const ROOT: &str = "/";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const BATTERY: &str = "org.bluez.Battery1";

#[derive(Debug, Clone)]
pub struct Device {
    pub name: String,
    pub address: String,
    pub signal: Option<i64>,
    pub battery: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct Radio {
    pub connected: Vec<Device>,
    pub paired: usize,
}

#[derive(Debug, Clone)]
pub struct Bluetooth {
    pub radio: Option<Radio>,
    pub adapter: Option<Adapter>,
}

impl Radio {
    #[must_use]
    pub fn best_signal(&self) -> Option<i64> {
        self.connected
            .iter()
            .filter_map(|device| device.signal)
            .max()
    }
}

fn reading(objects: &ManagedObjects) -> Option<Bluetooth> {
    let powered = objects
        .values()
        .filter_map(|interfaces| interface(interfaces, ADAPTER))
        .map(|adapter| prop::<bool>(adapter, "Powered").unwrap_or(false))
        .reduce(|any, powered| any || powered)?;
    let adapter = objects
        .iter()
        .filter(|(_, interfaces)| interface(interfaces, ADAPTER).is_some())
        .find_map(|(path, _)| Adapter::of_bluetooth_controller(path.as_str().rsplit('/').next()?));
    if !powered {
        return Some(Bluetooth {
            radio: None,
            adapter,
        });
    }

    let paired = objects
        .values()
        .filter_map(|interfaces| interface(interfaces, DEVICE))
        .filter(|device| prop::<bool>(device, "Paired").unwrap_or(false))
        .count();

    let connected = objects
        .values()
        .filter_map(|interfaces| {
            let device = interface(interfaces, DEVICE)?;
            if !prop::<bool>(device, "Connected").unwrap_or(false) {
                return None;
            }
            let address = prop::<String>(device, "Address")?;
            let name = prop::<String>(device, "Alias")
                .or_else(|| prop::<String>(device, "Name"))
                .unwrap_or_else(|| address.clone());
            let signal =
                prop::<i16>(device, "RSSI").map(|dbm| (i64::from(dbm).clamp(-90, -40) + 90) * 2);
            let battery = interface(interfaces, BATTERY)
                .and_then(|battery| prop::<u8>(battery, "Percentage"));
            Some(Device {
                name,
                address,
                signal,
                battery,
            })
        })
        .collect();

    Some(Bluetooth {
        radio: Some(Radio { connected, paired }),
        adapter,
    })
}

impl ReadingSource for Bluetooth {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = NAME;
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(system.clone(), SERVICE, move || {
            managed_objects_readings(system.clone(), SERVICE, ROOT, reading)
        })
    }
}
