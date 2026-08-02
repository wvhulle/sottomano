use iced::futures::Stream;
use zbus::fdo::ManagedObjects;

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody, Unit, Value,
    font_awesome::Brands,
    net_interfaces::{interface, managed_objects_readings, prop, signal_level},
    row,
};

pub const NAME: &str = "bluetooth";

const SERVICE: &str = "org.bluez";
const ROOT: &str = "/";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";

#[derive(Debug, Clone)]
pub struct Bluetooth {
    powered: bool,
    connected: Vec<String>,
    signal: Option<i64>,
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
        self.signal.map_or(Level::Ok, signal_level)
    }

    fn value(&self) -> Option<Value> {
        self.signal.map(|percent| Value::Gauge {
            percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        if !self.powered {
            return TooltipBody::Table(vec![row("Bluetooth", "off")]);
        }
        let summary = if self.connected.is_empty() {
            "on".to_string()
        } else {
            format!("{} connected", self.connected.len())
        };
        let mut rows = vec![row("Bluetooth", summary)];
        for (index, name) in self.connected.iter().enumerate() {
            rows.push(row(&format!("Device {}", index + 1), name.clone()));
        }
        TooltipBody::Table(rows)
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
            signal: None,
        });
    }

    let devices = objects
        .values()
        .filter_map(|interfaces| interface(interfaces, DEVICE))
        .filter(|device| prop::<bool>(device, "Connected").unwrap_or(false));
    let connected = devices
        .clone()
        .map(|device| {
            prop::<String>(device, "Alias")
                .or_else(|| prop::<String>(device, "Name"))
                .unwrap_or_default()
        })
        .collect();
    let signal = devices
        .filter_map(|device| prop::<i16>(device, "RSSI"))
        .map(|dbm| rssi_quality(i64::from(dbm)))
        .max();
    Some(Bluetooth {
        powered: true,
        connected,
        signal,
    })
}

pub fn readings() -> impl Stream<Item = Option<Bluetooth>> {
    managed_objects_readings(NAME, SERVICE, ROOT, reading)
}
