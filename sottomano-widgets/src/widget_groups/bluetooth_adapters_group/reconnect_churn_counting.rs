use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, TimeDelta, Utc};
use sottomano_sources::network_connectivity::bluez_bluetooth::Bluetooth;

pub const RECONNECT_HORIZON: TimeDelta = TimeDelta::minutes(10);

#[derive(Debug, Default)]
pub struct ReconnectHistory {
    ever_connected: BTreeSet<String>,
    connected_now: BTreeSet<String>,
    reconnected_at: BTreeMap<String, Vec<DateTime<Utc>>>,
}

impl ReconnectHistory {
    pub fn observe(&mut self, bluetooth: &Bluetooth, now: DateTime<Utc>) {
        let connected_addresses: BTreeSet<String> = bluetooth
            .connected_devices()
            .map(|device| device.address.to_lowercase())
            .collect();
        let returned_addresses: Vec<String> = connected_addresses
            .difference(&self.connected_now)
            .filter(|address| self.ever_connected.contains(*address))
            .cloned()
            .collect();
        returned_addresses
            .into_iter()
            .for_each(|address| self.reconnected_at.entry(address).or_default().push(now));
        self.reconnected_at.values_mut().for_each(|moments| {
            moments.retain(|moment| now - *moment <= RECONNECT_HORIZON);
        });
        self.reconnected_at.retain(|_, moments| !moments.is_empty());
        self.ever_connected
            .extend(connected_addresses.iter().cloned());
        self.connected_now = connected_addresses;
    }

    #[must_use]
    pub fn recent_reconnects(&self, address: &str, now: DateTime<Utc>) -> usize {
        self.reconnected_at
            .get(&address.to_lowercase())
            .map_or(0, |moments| {
                moments
                    .iter()
                    .filter(|moment| now - **moment <= RECONNECT_HORIZON)
                    .count()
            })
    }
}

#[cfg(test)]
mod tests {
    use sottomano_sources::network_connectivity::bluez_bluetooth::{
        AdapterPlacement, AdapterPower, BluetoothAdapter, ConnectedDevice, DeviceBattery,
        DeviceKind, PoweredAdapter,
    };
    use zbus::zvariant::OwnedObjectPath;

    use super::*;

    const PAD_ADDRESS: &str = "D4:2F:4B:69:6B:2F";

    fn adapter_with(addresses: &[&str]) -> Bluetooth {
        let connected = addresses
            .iter()
            .map(|address| ConnectedDevice {
                object_path: OwnedObjectPath::try_from("/org/bluez/hci0/dev_pad").unwrap(),
                name: "Pad".to_owned(),
                address: (*address).to_owned(),
                kind: DeviceKind::Gamepad,
                battery: DeviceBattery::Unreported,
                kernel_devices: Vec::new(),
            })
            .collect();
        Bluetooth {
            adapters: vec![BluetoothAdapter {
                object_path: OwnedObjectPath::try_from("/org/bluez/hci0").unwrap(),
                controller_name: "hci0".to_owned(),
                power: AdapterPower::On(PoweredAdapter { connected }),
                placement: AdapterPlacement::BuiltIn,
            }],
        }
    }

    fn minutes_after_start(minutes: i64) -> DateTime<Utc> {
        DateTime::<Utc>::UNIX_EPOCH + TimeDelta::minutes(minutes)
    }

    #[test]
    fn devices_connected_at_startup_are_not_reconnects() {
        let mut history = ReconnectHistory::default();
        history.observe(&adapter_with(&[PAD_ADDRESS]), minutes_after_start(0));
        assert_eq!(
            history.recent_reconnects(PAD_ADDRESS, minutes_after_start(0)),
            0
        );
    }

    #[test]
    fn a_drop_and_return_counts_once_until_the_horizon_passes() {
        let mut history = ReconnectHistory::default();
        history.observe(&adapter_with(&[PAD_ADDRESS]), minutes_after_start(0));
        history.observe(&adapter_with(&[]), minutes_after_start(1));
        history.observe(&adapter_with(&[PAD_ADDRESS]), minutes_after_start(2));
        history.observe(&adapter_with(&[PAD_ADDRESS]), minutes_after_start(3));
        assert_eq!(
            history.recent_reconnects(&PAD_ADDRESS.to_lowercase(), minutes_after_start(3)),
            1
        );
        assert_eq!(
            history.recent_reconnects(PAD_ADDRESS, minutes_after_start(13)),
            0
        );
    }
}
