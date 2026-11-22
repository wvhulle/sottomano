use std::{collections::HashMap, time::Duration};

use futures::{Stream, StreamExt, stream, try_join};
use zbus::Connection;

use super::{
    Peer, StatisticsAccess, Tunnel, TunnelActivity, TunnelOrigin, TunnelRole, Wireguard,
    wireguard_kernel_dumps::{KernelTunnelStatus, kernel_wireguard_interfaces, read_kernel_tunnel},
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::network_manager_proxies::{
        self, ActivationState, ActiveWireguard, WireguardProfile,
    },
    system_protocol_clients::{
        dbus_desktop_buses::{DesktopBuses, SERVICE_ABSENT_ERRORS},
        protocol_peer_reconnects::subscribed,
    },
};

const KERNEL_TUNNEL_READS_IN_FLIGHT: usize = 4;
const STATISTICS_RECHECK: Duration = Duration::from_secs(10);

impl Tunnel {
    fn observed_profile(
        profile: WireguardProfile,
        active_wireguards: &[ActiveWireguard],
        kernel_tunnel_statuses: &HashMap<String, KernelTunnelStatus>,
    ) -> Self {
        let activity = active_wireguards
            .iter()
            .find(|active| active.settings_connection == profile.settings_connection)
            .map_or(TunnelActivity::Inactive, |active| {
                let active_connection = active.active_connection.clone();
                match active.activation_state {
                    ActivationState::Activated => TunnelActivity::Active { active_connection },
                    ActivationState::Switching => TunnelActivity::Switching { active_connection },
                }
            });
        let configured_peers = profile.peers.into_iter().map(Peer::configured);
        let (statistics_access, peers) = match kernel_tunnel_statuses.get(&profile.interface) {
            Some(KernelTunnelStatus::Dumped(kernel_tunnel)) => (
                StatisticsAccess::Read,
                configured_peers
                    .map(|peer| peer.observed_by(&kernel_tunnel.peers))
                    .collect(),
            ),
            Some(KernelTunnelStatus::Unreadable(access)) => {
                (access.clone(), configured_peers.collect())
            }
            None => (StatisticsAccess::TunnelDown, configured_peers.collect()),
        };
        Self {
            role: TunnelRole::configured(profile.listen_port),
            origin: TunnelOrigin::NetworkManager {
                settings_connection: profile.settings_connection,
                activity,
            },
            name: profile.name,
            interface: profile.interface,
            statistics_access,
            peers,
        }
    }

    fn kernel_only(interface: String, sample: KernelTunnelStatus) -> Self {
        let (role, statistics_access, peers) = match sample {
            KernelTunnelStatus::Dumped(kernel_tunnel) => (
                TunnelRole::inferred_from_peers(kernel_tunnel.listen_port, &kernel_tunnel.peers),
                StatisticsAccess::Read,
                kernel_tunnel.peers,
            ),
            KernelTunnelStatus::Unreadable(access) => {
                (TunnelRole::Undetermined, access, Vec::new())
            }
        };
        Self {
            name: interface.clone(),
            interface,
            role,
            origin: TunnelOrigin::KernelOnly,
            statistics_access,
            peers,
        }
    }
}

const WATCHED_WIREGUARD_PROPERTIES: &[&str] = &[
    network_manager_proxies::MANAGER_PROPERTIES,
    network_manager_proxies::SETTINGS_PROPERTIES,
    network_manager_proxies::ACTIVE_CONNECTION_PROPERTIES,
];

impl SnapshotSource for Wireguard {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "WireGuard tunnel list";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let signal_connection = buses.system.clone();
        let read_connection = buses.system;
        PeriodicPolling::every(STATISTICS_RECHECK).triggered_deliveries(
            subscribed(signal_connection, |signal_connection| async move {
                network_manager_proxies::changes(&signal_connection, WATCHED_WIREGUARD_PROPERTIES)
                    .await
            }),
            move || read_wireguard(read_connection.clone()),
        )
    }
}

async fn read_wireguard(system: Connection) -> zbus::Result<Wireguard> {
    let (profiles, active_wireguards) = match try_join!(
        network_manager_proxies::wireguard_profiles(&system),
        network_manager_proxies::active_wireguards(&system)
    ) {
        Ok(network_manager_tunnels) => network_manager_tunnels,
        Err(zbus::Error::MethodError(name, _, _))
            if SERVICE_ABSENT_ERRORS.contains(&name.as_str()) =>
        {
            (Vec::new(), Vec::new())
        }
        Err(bus_error) => return Err(bus_error),
    };
    let kernel_tunnel_statuses = stream::iter(kernel_wireguard_interfaces().await)
        .map(|interface| async move {
            let sample = read_kernel_tunnel(&interface).await;
            (interface, sample)
        })
        .buffered(KERNEL_TUNNEL_READS_IN_FLIGHT)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect();
    Ok(assemble_tunnels(
        profiles,
        &active_wireguards,
        kernel_tunnel_statuses,
    ))
}

pub(super) fn assemble_tunnels(
    profiles: Vec<WireguardProfile>,
    active_wireguards: &[ActiveWireguard],
    mut kernel_tunnel_statuses: HashMap<String, KernelTunnelStatus>,
) -> Wireguard {
    let profile_tunnels: Vec<Tunnel> = profiles
        .into_iter()
        .map(|profile| {
            Tunnel::observed_profile(profile, active_wireguards, &kernel_tunnel_statuses)
        })
        .collect();
    for tunnel in &profile_tunnels {
        kernel_tunnel_statuses.remove(&tunnel.interface);
    }
    let mut kernel_only_tunnels: Vec<Tunnel> = kernel_tunnel_statuses
        .into_iter()
        .map(|(interface, sample)| Tunnel::kernel_only(interface, sample))
        .collect();
    kernel_only_tunnels.sort_by(|left, right| left.interface.cmp(&right.interface));
    Wireguard {
        tunnels: profile_tunnels
            .into_iter()
            .chain(kernel_only_tunnels)
            .collect(),
    }
}
