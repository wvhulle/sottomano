use std::{
    collections::HashMap,
    future::ready,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytesize::ByteSize;
use futures::{Stream, StreamExt, future::join_all, stream, try_join};
use serde_json::Value as Json;
use zbus::{Connection, zvariant::OwnedObjectPath};

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::network_manager_proxies::{
        self, ActivationState, ActiveWireguard, WireguardPeerSettings, WireguardProfile,
    },
    stream_operators::{read_once_then_per_burst, resubscribed},
    system_protocol_clients::{
        dbus_desktop_buses::{DesktopBuses, SERVICE_ABSENT_ERRORS},
        shell_command_output,
    },
};

const CONNECTED_WINDOW: Duration = Duration::from_secs(180);
const STATISTICS_RECHECK: Duration = Duration::from_secs(10);
const PERMISSION_DENIED_MESSAGE: &str = "Operation not permitted";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEndpoint {
    Roaming,
    At(String),
}

impl PeerEndpoint {
    fn from_reported(reported: Option<String>) -> Self {
        reported.map_or(Self::Roaming, Self::At)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerStatistics {
    pub handshake_ago: Duration,
    pub received: ByteSize,
    pub sent: ByteSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerTraffic {
    Unobserved,
    NeverHandshaken,
    Exchanged(PeerStatistics),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerLink {
    Connected,
    Stale,
}

impl PeerTraffic {
    #[must_use]
    pub fn peer_link(self) -> PeerLink {
        match self {
            Self::Exchanged(statistics) if statistics.handshake_ago <= CONNECTED_WINDOW => {
                PeerLink::Connected
            }
            Self::Exchanged(_) | Self::Unobserved | Self::NeverHandshaken => PeerLink::Stale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteScope {
    AllTraffic,
    SingleHosts,
    Subnets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub public_key: String,
    pub endpoint: PeerEndpoint,
    pub allowed_ips: Vec<String>,
    pub traffic: PeerTraffic,
}

impl Peer {
    #[must_use]
    pub fn route_scope(&self) -> RouteScope {
        if self
            .allowed_ips
            .iter()
            .any(|allowed_ip| matches!(allowed_ip.as_str(), "0.0.0.0/0" | "::/0"))
        {
            RouteScope::AllTraffic
        } else if !self.allowed_ips.is_empty()
            && self
                .allowed_ips
                .iter()
                .all(|allowed_ip| allowed_ip.ends_with("/32") || allowed_ip.ends_with("/128"))
        {
            RouteScope::SingleHosts
        } else {
            RouteScope::Subnets
        }
    }

    fn configured(peer_settings: WireguardPeerSettings) -> Self {
        Self {
            public_key: peer_settings.public_key,
            endpoint: PeerEndpoint::from_reported(peer_settings.endpoint),
            allowed_ips: peer_settings.allowed_ips,
            traffic: PeerTraffic::Unobserved,
        }
    }

    fn observed_by(self, kernel_peers: &[Peer]) -> Self {
        let Some(kernel_peer) = kernel_peers
            .iter()
            .find(|kernel_peer| kernel_peer.public_key == self.public_key)
        else {
            return self;
        };
        Self {
            endpoint: match &kernel_peer.endpoint {
                PeerEndpoint::At(_) => kernel_peer.endpoint.clone(),
                PeerEndpoint::Roaming => self.endpoint,
            },
            traffic: kernel_peer.traffic,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelRole {
    Server { listen_port: u32 },
    Client,
    Undetermined,
}

impl TunnelRole {
    fn from_configured_port(listen_port: u32) -> Self {
        if listen_port > 0 {
            Self::Server { listen_port }
        } else {
            Self::Client
        }
    }

    fn inferred_from_peers(listen_port: u32, peers: &[Peer]) -> Self {
        if peers.len() > 1
            || peers
                .first()
                .is_some_and(|peer| peer.route_scope() == RouteScope::SingleHosts)
        {
            Self::Server { listen_port }
        } else {
            Self::Client
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelActivity {
    Inactive,
    Switching { active_connection: OwnedObjectPath },
    Active { active_connection: OwnedObjectPath },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelOrigin {
    NetworkManager {
        settings_connection: OwnedObjectPath,
        activity: TunnelActivity,
    },
    KernelOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatisticsAccess {
    TunnelDown,
    NeedsNetAdmin,
    Unreadable(String),
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireguardSwitch {
    Connect(OwnedObjectPath),
    Disconnect(OwnedObjectPath),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tunnel {
    pub name: String,
    pub interface: String,
    pub role: TunnelRole,
    pub origin: TunnelOrigin,
    pub statistics_access: StatisticsAccess,
    pub peers: Vec<Peer>,
}

impl Tunnel {
    #[must_use]
    pub fn connected_peers(&self) -> usize {
        self.peers
            .iter()
            .filter(|peer| peer.traffic.peer_link() == PeerLink::Connected)
            .count()
    }

    #[must_use]
    pub fn tunnel_phase(&self) -> TunnelPhase {
        match &self.origin {
            TunnelOrigin::NetworkManager {
                activity: TunnelActivity::Active { .. },
                ..
            }
            | TunnelOrigin::KernelOnly => TunnelPhase::Up,
            TunnelOrigin::NetworkManager {
                activity: TunnelActivity::Switching { .. },
                ..
            } => TunnelPhase::Switching,
            TunnelOrigin::NetworkManager {
                activity: TunnelActivity::Inactive,
                ..
            } => TunnelPhase::Down,
        }
    }

    #[must_use]
    pub fn traffic_routing(&self) -> TunnelRouting {
        let default_route = self.role == TunnelRole::Client
            && self.tunnel_phase() == TunnelPhase::Up
            && self
                .peers
                .iter()
                .any(|peer| peer.route_scope() == RouteScope::AllTraffic);
        if default_route {
            TunnelRouting::DefaultRoute
        } else {
            TunnelRouting::Partial
        }
    }

    #[must_use]
    pub fn switch(&self) -> Option<WireguardSwitch> {
        let TunnelOrigin::NetworkManager {
            settings_connection,
            activity,
        } = &self.origin
        else {
            return None;
        };
        Some(match activity {
            TunnelActivity::Inactive => WireguardSwitch::Connect(settings_connection.clone()),
            TunnelActivity::Switching { active_connection }
            | TunnelActivity::Active { active_connection } => {
                WireguardSwitch::Disconnect(active_connection.clone())
            }
        })
    }

    fn from_profile(
        profile: WireguardProfile,
        active_wireguards: &[ActiveWireguard],
        kernel_readings: &HashMap<String, KernelReading>,
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
        let (statistics_access, peers) = match kernel_readings.get(&profile.interface) {
            Some(KernelReading::Dumped(kernel_tunnel)) => (
                StatisticsAccess::Read,
                configured_peers
                    .map(|peer| peer.observed_by(&kernel_tunnel.peers))
                    .collect(),
            ),
            Some(KernelReading::Unreadable(access)) => (access.clone(), configured_peers.collect()),
            None => (StatisticsAccess::TunnelDown, configured_peers.collect()),
        };
        Self {
            role: TunnelRole::from_configured_port(profile.listen_port),
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

    fn kernel_only(interface: String, reading: KernelReading) -> Self {
        let (role, statistics_access, peers) = match reading {
            KernelReading::Dumped(kernel_tunnel) => (
                TunnelRole::inferred_from_peers(kernel_tunnel.listen_port, &kernel_tunnel.peers),
                StatisticsAccess::Read,
                kernel_tunnel.peers,
            ),
            KernelReading::Unreadable(access) => (TunnelRole::Undetermined, access, Vec::new()),
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

#[derive(Debug, Clone, PartialEq)]
pub struct TunnelAddress {
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TunnelOnBus {
    pub buses: DesktopBuses,
    pub tunnel: String,
}

impl SnapshotSource for TunnelAddress {
    type Error = shell_command_output::Error;
    type StreamInputs = TunnelOnBus;

    const NAME: &'static str = "Tunnel address";

    fn delivery_stream(watched: TunnelOnBus) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(address_changes(watched))
    }
}

async fn address_changes(
    watched: TunnelOnBus,
) -> Result<
    impl Stream<Item = Result<TunnelAddress, shell_command_output::Error>> + Send,
    shell_command_output::Error,
> {
    let addressing = match network_manager_proxies::changes(
        &watched.buses.system,
        ADDRESSING_PROPERTIES,
    )
    .await
    {
        Ok(signals) => signals.left_stream(),
        Err(e) => {
            tracing::debug!("wireguard: no NetworkManager to follow: {e}");
            stream::empty().right_stream()
        }
    };
    Ok(read_once_then_per_burst(addressing, move || {
        let tunnel = watched.tunnel.clone();
        async move {
            Ok(TunnelAddress {
                ip: interface_ip(&tunnel).await,
            })
        }
    }))
}

const ADDRESSING_PROPERTIES: &[&str] = &[
    network_manager_proxies::DEVICE_PROPERTIES,
    network_manager_proxies::IP4_CONFIG_PROPERTIES,
];

const WATCHED_WIREGUARD_PROPERTIES: &[&str] = &[
    network_manager_proxies::MANAGER_PROPERTIES,
    network_manager_proxies::SETTINGS_PROPERTIES,
    network_manager_proxies::ACTIVE_CONNECTION_PROPERTIES,
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wireguard {
    pub tunnels: Vec<Tunnel>,
}

impl SnapshotSource for Wireguard {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "WireGuard";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let signal_connection = buses.system.clone();
        let read_connection = buses.system;
        PeriodicPolling::every(STATISTICS_RECHECK).triggered_deliveries(
            resubscribed(Duration::from_secs(1), move || {
                let signal_connection = signal_connection.clone();
                async move {
                    Ok(network_manager_proxies::changes(
                        &signal_connection,
                        WATCHED_WIREGUARD_PROPERTIES,
                    )
                    .await?
                    .map(Ok::<_, zbus::Error>))
                }
            })
            .filter_map(|signal| ready(signal.ok())),
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
    let kernel_readings = join_all(kernel_wireguard_interfaces().await.into_iter().map(
        |interface| async move {
            let reading = read_kernel_tunnel(&interface).await;
            (interface, reading)
        },
    ))
    .await
    .into_iter()
    .collect();
    Ok(assemble_tunnels(
        profiles,
        &active_wireguards,
        kernel_readings,
    ))
}

fn assemble_tunnels(
    profiles: Vec<WireguardProfile>,
    active_wireguards: &[ActiveWireguard],
    mut kernel_readings: HashMap<String, KernelReading>,
) -> Wireguard {
    let profile_tunnels: Vec<Tunnel> = profiles
        .into_iter()
        .map(|profile| Tunnel::from_profile(profile, active_wireguards, &kernel_readings))
        .collect();
    for tunnel in &profile_tunnels {
        kernel_readings.remove(&tunnel.interface);
    }
    let mut kernel_only_tunnels: Vec<Tunnel> = kernel_readings
        .into_iter()
        .map(|(interface, reading)| Tunnel::kernel_only(interface, reading))
        .collect();
    kernel_only_tunnels.sort_by(|left, right| left.interface.cmp(&right.interface));
    Wireguard {
        tunnels: profile_tunnels
            .into_iter()
            .chain(kernel_only_tunnels)
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KernelTunnel {
    listen_port: u32,
    peers: Vec<Peer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum KernelReading {
    Dumped(KernelTunnel),
    Unreadable(StatisticsAccess),
}

async fn kernel_wireguard_interfaces() -> Vec<String> {
    match shell_command_output::output("ip", &["-j", "link", "show", "type", "wireguard"]).await {
        Ok(listing) => wireguard_interface_names(&listing),
        Err(listing_error) => {
            tracing::debug!("wireguard: cannot list kernel interfaces: {listing_error}");
            Vec::new()
        }
    }
}

fn wireguard_interface_names(listing: &str) -> Vec<String> {
    serde_json::from_str::<Json>(listing)
        .ok()
        .and_then(|json| {
            json.as_array().map(|links| {
                links
                    .iter()
                    .filter_map(|link| link.get("ifname")?.as_str().map(ToOwned::to_owned))
                    .collect()
            })
        })
        .unwrap_or_default()
}

async fn read_kernel_tunnel(interface: &str) -> KernelReading {
    match shell_command_output::output("wg", &["show", interface, "dump"]).await {
        Ok(dump) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since_epoch| since_epoch.as_secs());
            parse_kernel_dump(&dump, now).map_or_else(
                || {
                    KernelReading::Unreadable(StatisticsAccess::Unreadable(format!(
                        "empty dump for {interface}"
                    )))
                },
                KernelReading::Dumped,
            )
        }
        Err(shell_command_output::Error::Failed { stderr, .. })
            if stderr.contains(PERMISSION_DENIED_MESSAGE) =>
        {
            KernelReading::Unreadable(StatisticsAccess::NeedsNetAdmin)
        }
        Err(dump_error) => {
            tracing::debug!("wireguard: cannot dump {interface}: {dump_error}");
            KernelReading::Unreadable(StatisticsAccess::Unreadable(dump_error.to_string()))
        }
    }
}

fn parse_kernel_dump(dump: &str, now: u64) -> Option<KernelTunnel> {
    let mut lines = dump.lines();
    let interface_line = lines.next()?;
    let listen_port = interface_line
        .split('\t')
        .nth(2)
        .and_then(|port| port.parse().ok())
        .unwrap_or_default();
    Some(KernelTunnel {
        listen_port,
        peers: lines.filter_map(|line| kernel_peer(line, now)).collect(),
    })
}

fn kernel_peer(dump_line: &str, now: u64) -> Option<Peer> {
    let fields: Vec<&str> = dump_line.split('\t').collect();
    let reported = |position: usize| {
        fields
            .get(position)
            .filter(|field| !matches!(**field, "(none)" | ""))
            .map(ToString::to_string)
    };
    let handshake: u64 = fields.get(4)?.parse().ok()?;
    let received = ByteSize::b(fields.get(5)?.parse().ok()?);
    let sent = ByteSize::b(fields.get(6)?.parse().ok()?);
    Some(Peer {
        public_key: reported(0)?,
        endpoint: PeerEndpoint::from_reported(reported(2)),
        allowed_ips: reported(3)
            .map(|allowed_ips| allowed_ips.split(',').map(ToOwned::to_owned).collect())
            .unwrap_or_default(),
        traffic: if handshake == 0 {
            PeerTraffic::NeverHandshaken
        } else {
            PeerTraffic::Exchanged(PeerStatistics {
                handshake_ago: Duration::from_secs(now.saturating_sub(handshake)),
                received,
                sent,
            })
        },
    })
}

async fn interface_ip(iface: &str) -> Option<String> {
    shell_command_output::output("ip", &["-j", "-4", "addr", "show", iface])
        .await
        .ok()
        .and_then(|out| serde_json::from_str::<Json>(&out).ok())
        .and_then(|json| {
            let entry = json.as_array()?.first()?;
            let addr = entry.get("addr_info")?.as_array()?.first()?;
            let local = addr.get("local")?.as_str()?;
            let prefix = addr.get("prefixlen").and_then(Json::as_u64).unwrap_or(32);
            Some(format!("{local}/{prefix}"))
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelPhase {
    Up,
    Switching,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelRouting {
    DefaultRoute,
    Partial,
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000;

    const SERVER_DUMP_ROWS: &[&[&str]] = &[
        &["private", "public", "51820", "off"],
        &[
            "laptop-key",
            "(none)",
            "198.51.100.7:40211",
            "172.20.10.2/32",
            "999958",
            "12582912",
            "3145728",
            "off",
        ],
        &[
            "phone-key",
            "(none)",
            "(none)",
            "172.20.10.3/32",
            "0",
            "0",
            "0",
            "off",
        ],
    ];

    const CLIENT_DUMP_ROWS: &[&[&str]] = &[
        &["private", "public", "43122", "off"],
        &[
            "server-key",
            "(none)",
            "203.0.113.24:51820",
            "0.0.0.0/0,::/0",
            "999963",
            "88080384",
            "12582912",
            "25",
        ],
    ];

    fn dump_text(dump_rows: &[&[&str]]) -> String {
        dump_rows
            .iter()
            .map(|dump_row| dump_row.join("\t") + "\n")
            .collect()
    }

    fn parsed_dump(dump_rows: &[&[&str]]) -> KernelTunnel {
        parse_kernel_dump(&dump_text(dump_rows), NOW).unwrap()
    }

    fn object_path(path: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(path).unwrap()
    }

    fn host_peer_settings(public_key: &str, allowed_ip: &str) -> WireguardPeerSettings {
        WireguardPeerSettings {
            public_key: public_key.to_owned(),
            endpoint: None,
            allowed_ips: vec![allowed_ip.to_owned()],
        }
    }

    fn server_profile() -> WireguardProfile {
        WireguardProfile {
            settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/3"),
            name: "wg-wss".to_owned(),
            interface: "wg-wss".to_owned(),
            listen_port: 51820,
            peers: vec![
                host_peer_settings("laptop-key", "172.20.10.2/32"),
                host_peer_settings("phone-key", "172.20.10.3/32"),
            ],
        }
    }

    fn active_server() -> ActiveWireguard {
        ActiveWireguard {
            settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/3"),
            active_connection: object_path("/org/freedesktop/NetworkManager/ActiveConnection/9"),
            activation_state: ActivationState::Activated,
        }
    }

    fn dumped(dump_rows: &[&[&str]]) -> KernelReading {
        KernelReading::Dumped(parsed_dump(dump_rows))
    }

    #[test]
    fn configured_listen_port_decides_role() {
        assert_eq!(
            TunnelRole::from_configured_port(51820),
            TunnelRole::Server { listen_port: 51820 }
        );
        assert_eq!(TunnelRole::from_configured_port(0), TunnelRole::Client);
    }

    #[test]
    fn single_peer_routing_everything_is_a_client() {
        let kernel_tunnel = parsed_dump(CLIENT_DUMP_ROWS);
        assert_eq!(
            TunnelRole::inferred_from_peers(kernel_tunnel.listen_port, &kernel_tunnel.peers),
            TunnelRole::Client
        );
    }

    #[test]
    fn host_route_peers_make_a_server() {
        let kernel_tunnel = parsed_dump(SERVER_DUMP_ROWS);
        assert_eq!(
            TunnelRole::inferred_from_peers(kernel_tunnel.listen_port, &kernel_tunnel.peers),
            TunnelRole::Server { listen_port: 51820 }
        );
    }

    #[test]
    fn dump_parses_peer_traffic() {
        let kernel_tunnel = parsed_dump(SERVER_DUMP_ROWS);
        assert_eq!(
            kernel_tunnel.peers[0].traffic,
            PeerTraffic::Exchanged(PeerStatistics {
                handshake_ago: Duration::from_secs(42),
                received: ByteSize::mib(12),
                sent: ByteSize::mib(3),
            })
        );
        assert_eq!(kernel_tunnel.peers[1].traffic, PeerTraffic::NeverHandshaken);
        assert_eq!(kernel_tunnel.peers[1].endpoint, PeerEndpoint::Roaming);
    }

    #[test]
    fn unprivileged_profile_keeps_configured_peers_unobserved() {
        let wireguard = assemble_tunnels(
            vec![server_profile()],
            &[active_server()],
            HashMap::from([(
                "wg-wss".to_owned(),
                KernelReading::Unreadable(StatisticsAccess::NeedsNetAdmin),
            )]),
        );
        let [tunnel] = wireguard.tunnels.as_slice() else {
            panic!("expected only the NetworkManager tunnel");
        };
        assert_eq!(tunnel.role, TunnelRole::Server { listen_port: 51820 });
        assert_eq!(tunnel.statistics_access, StatisticsAccess::NeedsNetAdmin);
        assert_eq!(tunnel.tunnel_phase(), TunnelPhase::Up);
        assert_eq!(tunnel.peers.len(), 2);
        assert!(
            tunnel
                .peers
                .iter()
                .all(|peer| peer.traffic == PeerTraffic::Unobserved)
        );
        assert_eq!(
            tunnel.switch(),
            Some(WireguardSwitch::Disconnect(object_path(
                "/org/freedesktop/NetworkManager/ActiveConnection/9"
            )))
        );
    }

    #[test]
    fn readable_dump_fills_profile_peer_traffic() {
        let wireguard = assemble_tunnels(
            vec![server_profile()],
            &[active_server()],
            HashMap::from([("wg-wss".to_owned(), dumped(SERVER_DUMP_ROWS))]),
        );
        let tunnel = &wireguard.tunnels[0];
        assert_eq!(tunnel.connected_peers(), 1);
        assert_eq!(
            tunnel.peers[0].endpoint,
            PeerEndpoint::At("198.51.100.7:40211".to_owned())
        );
    }

    #[test]
    fn inactive_profile_switches_to_connect() {
        let wireguard = assemble_tunnels(vec![server_profile()], &[], HashMap::new());
        let tunnel = &wireguard.tunnels[0];
        assert_eq!(tunnel.tunnel_phase(), TunnelPhase::Down);
        assert_eq!(tunnel.statistics_access, StatisticsAccess::TunnelDown);
        assert_eq!(
            tunnel.switch(),
            Some(WireguardSwitch::Connect(object_path(
                "/org/freedesktop/NetworkManager/Settings/3"
            )))
        );
    }

    #[test]
    fn kernel_only_tunnel_has_no_switch() {
        let wireguard = assemble_tunnels(
            Vec::new(),
            &[],
            HashMap::from([("wg0".to_owned(), dumped(CLIENT_DUMP_ROWS))]),
        );
        let tunnel = &wireguard.tunnels[0];
        assert_eq!(tunnel.origin, TunnelOrigin::KernelOnly);
        assert_eq!(tunnel.switch(), None);
        assert_eq!(tunnel.traffic_routing(), TunnelRouting::DefaultRoute);
    }

    #[test]
    fn ip_link_listing_yields_interface_names() {
        let listing = r#"[{"ifindex":7,"ifname":"wg-wss","flags":["POINTOPOINT","NOARP","UP"]}]"#;
        assert_eq!(wireguard_interface_names(listing), ["wg-wss"]);
    }
}
