use sottomano_desktop_sources::{
    SnapshotDelivery,
    network_connectivity::{
        ethernet_connection::Ethernet,
        network_manager_proxies::NetworkName,
        wifi_connection::{AccessPointAssociation, Wifi},
    },
};
use uom::si::f64::Frequency;

use super::{
    GamepadGroup,
    controller_link_indicator::{WiredFallback, wifi_band},
};
use crate::panel_menus::gaming_menu::gamepad_group::controller_link_indicator::WifiBand;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) enum WifiBandUse {
    #[default]
    ClearOfBluetooth,
    SharingBluetoothBand {
        network_name: String,
        frequency: Frequency,
    },
}

impl From<&Wifi> for WifiBandUse {
    fn from(wifi: &Wifi) -> Self {
        wifi.adapters
            .iter()
            .filter_map(|adapter| match &adapter.link {
                AccessPointAssociation::Associated(link) => Some(link),
                AccessPointAssociation::Unassociated => None,
            })
            .fold(WifiBandUse::ClearOfBluetooth, |band_use, link| {
                match (band_use, wifi_band(link.frequency)) {
                    (WifiBandUse::ClearOfBluetooth, WifiBand::SharedWithBluetooth) => {
                        WifiBandUse::SharingBluetoothBand {
                            network_name: match &link.ssid {
                                NetworkName::Broadcast(ssid) => ssid.clone(),
                                NetworkName::Hidden => "hidden network".to_owned(),
                            },
                            frequency: link.frequency,
                        }
                    }
                    (band_use, _) => band_use,
                }
            })
    }
}

impl GamepadGroup {
    pub(super) fn receive_wifi_delivery(&mut self, delivery: SnapshotDelivery<Wifi>) {
        match delivery {
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Ok(wifi),
                ..
            } => self.wifi_band_use = WifiBandUse::from(&wifi),
            SnapshotDelivery::SourceUnavailable
            | SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Err(_),
                ..
            } => self.wifi_band_use = WifiBandUse::ClearOfBluetooth,
            SnapshotDelivery::SnapshotUnchanged => {}
        }
    }

    pub(super) fn receive_ethernet_delivery(&mut self, delivery: SnapshotDelivery<Ethernet>) {
        match delivery {
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Ok(ethernet),
                ..
            } => {
                self.wired_fallback = if ethernet.connected_count() > 0 {
                    WiredFallback::Connected
                } else {
                    WiredFallback::Absent
                };
            }
            SnapshotDelivery::SourceUnavailable
            | SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Err(_),
                ..
            } => self.wired_fallback = WiredFallback::Absent,
            SnapshotDelivery::SnapshotUnchanged => {}
        }
    }
}
