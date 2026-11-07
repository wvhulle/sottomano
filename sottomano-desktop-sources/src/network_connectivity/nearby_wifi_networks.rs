use std::collections::HashMap;

use futures::{Stream, future::try_join_all, try_join};
use zbus::{Connection, zvariant::OwnedObjectPath};

use crate::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::{
        ReadingError, device_or_absent,
        network_manager_proxies::{self, ApInfo, DeviceInfo, WirelessProfile},
        nm_readings,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleNetwork {
    pub ssid: String,
    pub signal: i64,
    pub security: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownNetwork {
    pub settings_connection: OwnedObjectPath,
    pub profile_name: String,
    pub visible: VisibleNetwork,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdapterNearbyNetworks {
    pub device: String,
    pub known: Vec<KnownNetwork>,
    pub unknown: Vec<VisibleNetwork>,
}

#[derive(Debug, Clone)]
pub struct NearbyWifiNetworks {
    pub adapters: Vec<AdapterNearbyNetworks>,
}

impl NearbyWifiNetworks {
    #[must_use]
    pub fn of_adapter(&self, adapter_device: &str) -> Option<&AdapterNearbyNetworks> {
        self.adapters
            .iter()
            .find(|adapter_networks| adapter_networks.device == adapter_device)
    }
}

fn strongest_network_per_ssid(access_points: Vec<ApInfo>) -> Vec<VisibleNetwork> {
    let mut strongest_by_ssid: HashMap<String, VisibleNetwork> = HashMap::new();
    access_points
        .into_iter()
        .filter_map(|access_point| {
            access_point.ssid.map(|ssid| VisibleNetwork {
                ssid,
                signal: access_point.signal,
                security: access_point.security,
            })
        })
        .for_each(|visible_network| {
            strongest_by_ssid
                .entry(visible_network.ssid.clone())
                .and_modify(|strongest| {
                    if visible_network.signal > strongest.signal {
                        *strongest = visible_network.clone();
                    }
                })
                .or_insert(visible_network);
        });
    let mut visible_networks: Vec<VisibleNetwork> = strongest_by_ssid.into_values().collect();
    visible_networks.sort_by(|left, right| {
        right
            .signal
            .cmp(&left.signal)
            .then_with(|| left.ssid.cmp(&right.ssid))
    });
    visible_networks
}

fn split_known_networks(
    device: String,
    access_points: Vec<ApInfo>,
    profiles: Vec<WirelessProfile>,
) -> AdapterNearbyNetworks {
    let (known, unknown) = strongest_network_per_ssid(access_points)
        .into_iter()
        .fold(
            (Vec::new(), Vec::new()),
            |(mut known, mut unknown), visible_network| {
                match profiles
                    .iter()
                    .find(|profile| profile.ssid == visible_network.ssid)
                {
                    Some(profile) => known.push(KnownNetwork {
                        settings_connection: profile.settings_connection.clone(),
                        profile_name: profile.profile_name.clone(),
                        visible: visible_network,
                    }),
                    None => unknown.push(visible_network),
                }
                (known, unknown)
            },
        );
    AdapterNearbyNetworks {
        device,
        known,
        unknown,
    }
}

async fn adapter_nearby_networks(
    connection: &Connection,
    device: DeviceInfo,
) -> zbus::Result<AdapterNearbyNetworks> {
    let (access_points, profiles) = try_join!(
        network_manager_proxies::visible_access_points(connection, &device),
        network_manager_proxies::available_wireless_profiles(connection, &device)
    )?;
    Ok(split_known_networks(device.interface, access_points, profiles))
}

async fn read_nearby_networks(connection: Connection) -> Result<NearbyWifiNetworks, ReadingError> {
    let devices = network_manager_proxies::devices_of_kind(
        &connection,
        network_manager_proxies::DeviceKind::Wifi,
    )
    .await?;
    let adapters = try_join_all(
        devices
            .into_iter()
            .map(|device| adapter_nearby_networks(&connection, device)),
    )
    .await?;
    Ok(NearbyWifiNetworks { adapters })
}

impl SnapshotSource for NearbyWifiNetworks {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "nearby Wi-Fi networks";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(
            system.clone(),
            network_manager_proxies::SERVICE,
            move || {
                nm_readings(
                    system.clone(),
                    &[
                        network_manager_proxies::DEVICE_PROPERTIES,
                        network_manager_proxies::WIRELESS_PROPERTIES,
                        network_manager_proxies::SETTINGS_PROPERTIES,
                    ],
                    read_nearby_networks,
                )
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use uom::si::{f64::Frequency, frequency::megahertz};
    use zbus::zvariant::OwnedObjectPath;

    use super::{ApInfo, WirelessProfile, split_known_networks};

    fn access_point(ssid: &str, signal: i64) -> ApInfo {
        ApInfo {
            ssid: Some(ssid.to_owned()),
            signal,
            frequency: Frequency::new::<megahertz>(2437.0),
            security: "WPA2".to_owned(),
        }
    }

    fn saved_profile(ssid: &str) -> WirelessProfile {
        WirelessProfile {
            settings_connection: OwnedObjectPath::try_from(
                "/org/freedesktop/NetworkManager/Settings/1",
            )
            .expect("valid object path"),
            profile_name: ssid.to_owned(),
            ssid: ssid.to_owned(),
        }
    }

    #[test]
    fn saved_ssids_are_known_and_the_rest_unknown_strongest_first() {
        let nearby_networks = split_known_networks(
            "wlan0".to_owned(),
            vec![
                access_point("Neighbour", 40),
                access_point("HomeNet", 30),
                access_point("HomeNet", 70),
                access_point("Cafe", 55),
            ],
            vec![saved_profile("HomeNet")],
        );
        let known_ssids: Vec<_> = nearby_networks
            .known
            .iter()
            .map(|known_network| (known_network.visible.ssid.as_str(), known_network.visible.signal))
            .collect();
        let unknown_ssids: Vec<_> = nearby_networks
            .unknown
            .iter()
            .map(|visible_network| visible_network.ssid.as_str())
            .collect();
        assert_eq!(known_ssids, [("HomeNet", 70)]);
        assert_eq!(unknown_ssids, ["Cafe", "Neighbour"]);
    }

    #[test]
    fn hidden_access_points_are_left_out() {
        let hidden_access_point = ApInfo {
            ssid: None,
            ..access_point("ignored", 90)
        };
        let nearby_networks =
            split_known_networks("wlan0".to_owned(), vec![hidden_access_point], Vec::new());
        assert!(nearby_networks.known.is_empty());
        assert!(nearby_networks.unknown.is_empty());
    }
}
