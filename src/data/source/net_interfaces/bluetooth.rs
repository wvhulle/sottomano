use iced::futures::Stream;
use zbus::fdo::ManagedObjects;

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody, Unit, Value,
    font_awesome::Brands,
    net_interfaces::{interface, managed_objects_readings, prop, signal_level},
    row, section,
};

pub const NAME: &str = "bluetooth";

const SERVICE: &str = "org.bluez";
const ROOT: &str = "/";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const BATTERY: &str = "org.bluez.Battery1";

#[derive(Debug, Clone)]
struct Device {
    name: String,
    address: Option<String>,
    signal: Option<i64>,
    battery: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct Bluetooth {
    powered: bool,
    connected: Vec<Device>,
    paired: usize,
}

fn rssi_quality(dbm: i64) -> i64 {
    (dbm.clamp(-90, -40) + 90) * 2
}

impl BarWidget for Bluetooth {
    fn icon(&self) -> Glyph {
        Brands::BluetoothB.into()
    }

    fn disabled(&self) -> bool {
        !self.powered
    }

    fn badge(&self) -> Option<u32> {
        if !self.powered {
            return None;
        }
        u32::try_from(self.connected.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        self.best_signal().map_or(Level::Ok, signal_level)
    }

    fn value(&self) -> Option<Value> {
        self.best_signal().map(|percent| Value::Gauge {
            percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        if !self.powered {
            return TooltipBody::rows(vec![row("Bluetooth", "off")]);
        }
        let mut sections = vec![section(
            "Bluetooth",
            vec![
                row("Connected", self.connected.len().to_string()),
                row("Paired", self.paired.to_string()),
            ],
        )];
        for device in &self.connected {
            let mut rows = Vec::new();
            if let Some(address) = &device.address {
                rows.push(row("Address", address.clone()));
            }
            if let Some(signal) = device.signal {
                rows.push(row("Signal", format!("{signal}%")));
            }
            if let Some(battery) = device.battery {
                rows.push(row("Battery", format!("{battery}%")));
            }
            sections.push(section(device.name.clone(), rows));
        }
        TooltipBody::Sections(sections)
    }
}

impl Bluetooth {
    fn best_signal(&self) -> Option<i64> {
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
    if !powered {
        return Some(Bluetooth {
            powered: false,
            connected: Vec::new(),
            paired: 0,
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
            let name = prop::<String>(device, "Alias")
                .or_else(|| prop::<String>(device, "Name"))
                .unwrap_or_default();
            let address = prop::<String>(device, "Address");
            let signal = prop::<i16>(device, "RSSI").map(|dbm| rssi_quality(i64::from(dbm)));
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
        powered: true,
        connected,
        paired,
    })
}

pub fn readings() -> impl Stream<Item = Option<Bluetooth>> {
    managed_objects_readings(NAME, SERVICE, ROOT, reading)
}
