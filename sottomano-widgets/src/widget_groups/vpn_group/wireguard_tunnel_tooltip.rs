use iced::alignment::Horizontal;
use sottomano_config::severity::Severity;
use sottomano_sources::network_connectivity::wireguard_tunnels::{
    Peer, PeerEndpoint, PeerTraffic, StatisticsAccess, Tunnel, TunnelAddress, TunnelRole, Wireguard,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        tooltip_body::{
            Cell, ColumnHeading, KeyValue, ReadingSections, Section, TableRecord, TableRow,
            TooltipBody,
        },
        widget_state::WidgetState,
    },
};

impl BarWidget for Wireguard {
    fn name(&self) -> &'static str {
        "WireGuard"
    }

    fn icon(&self) -> Glyph {
        SimpleIcon::WireGuard.into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}

fn endpoint_cell(endpoint: &PeerEndpoint) -> Option<Cell> {
    match endpoint {
        PeerEndpoint::At(address) => Some(address.as_str().into()),
        PeerEndpoint::Roaming => None,
    }
}

fn handshake_cell(traffic: PeerTraffic) -> Option<Cell> {
    match traffic {
        PeerTraffic::Exchanged(statistics) => Some(Cell::duration(statistics.handshake_ago)),
        PeerTraffic::NeverHandshaken => Some("never".into()),
        PeerTraffic::Unobserved => None,
    }
}

fn transfer_cells(traffic: PeerTraffic) -> [Option<Cell>; 2] {
    match traffic {
        PeerTraffic::Exchanged(statistics) => [
            Some(Quantity::size(statistics.received).into()),
            Some(Quantity::size(statistics.sent).into()),
        ],
        PeerTraffic::NeverHandshaken | PeerTraffic::Unobserved => [None, None],
    }
}

fn allowed_ips_text(peer: &Peer) -> String {
    if peer.allowed_ips.is_empty() {
        "no allowed IPs".to_owned()
    } else {
        peer.allowed_ips.join(", ")
    }
}

const PUBLIC_KEY_PREFIX_LENGTH: usize = 8;

fn peer_label(peer: &Peer) -> String {
    if peer.allowed_ips.is_empty() {
        let key_prefix: String = peer
            .public_key
            .chars()
            .take(PUBLIC_KEY_PREFIX_LENGTH)
            .collect();
        format!("{key_prefix}…")
    } else {
        peer.allowed_ips.join(", ")
    }
}

impl TableRow<4> for &Peer {
    fn table_record(self) -> TableRecord<4> {
        let [received, sent] = transfer_cells(self.traffic);
        TableRecord::new(
            peer_label(self),
            [
                endpoint_cell(&self.endpoint),
                handshake_cell(self.traffic),
                received,
                sent,
            ],
        )
    }
}

fn statistics_entry(statistics_access: &StatisticsAccess) -> Option<KeyValue> {
    match statistics_access {
        StatisticsAccess::NeedsNetAdmin => Some(KeyValue::new("Peer stats", "need CAP_NET_ADMIN")),
        StatisticsAccess::Unreadable(detail) => {
            Some(KeyValue::new("Peer stats", detail.as_str()).level(Severity::Warning))
        }
        StatisticsAccess::TunnelDown | StatisticsAccess::Read => None,
    }
}

fn activity_entry(tunnel: &Tunnel) -> Option<KeyValue> {
    if tunnel.switching() {
        Some(KeyValue::new("State", "switching"))
    } else if tunnel.up() {
        None
    } else {
        Some(KeyValue::new("State", "not connected"))
    }
}

fn clients_entry(tunnel: &Tunnel) -> KeyValue {
    let clients = match tunnel.statistics_access {
        StatisticsAccess::Read => format!(
            "{}/{} connected",
            tunnel.connected_peers(),
            tunnel.peers.len()
        ),
        StatisticsAccess::TunnelDown
        | StatisticsAccess::NeedsNetAdmin
        | StatisticsAccess::Unreadable(_) => format!("{} configured", tunnel.peers.len()),
    };
    KeyValue::new("Clients", clients)
}

fn server_tooltip(tunnel: &Tunnel, listen_port: u32) -> TooltipBody {
    let summary_section = Section::dictionary(
        [
            KeyValue::new(tunnel.name.as_str(), "server"),
            KeyValue::new("Listen port", listen_port.to_string()),
            clients_entry(tunnel),
        ]
        .into_iter()
        .chain(activity_entry(tunnel))
        .chain(statistics_entry(&tunnel.statistics_access)),
    );
    let peer_table = Section::column_table(
        [
            ColumnHeading::titled("Endpoint").align_x(Horizontal::Left),
            ColumnHeading::titled("Handshake"),
            ColumnHeading::titled("Received"),
            ColumnHeading::titled("Sent"),
        ],
        &tunnel.peers,
    );
    TooltipBody::sections([summary_section, peer_table])
}

fn client_tooltip(tunnel: &Tunnel) -> TooltipBody {
    let peer_entries = tunnel.peers.iter().flat_map(|peer| {
        let [received, sent] = transfer_cells(peer.traffic);
        [
            endpoint_cell(&peer.endpoint).map(|endpoint| KeyValue::new("Server", endpoint)),
            Some(KeyValue::new("Allowed IPs", allowed_ips_text(peer))),
            handshake_cell(peer.traffic).map(|handshake| KeyValue::new("Handshake", handshake)),
            received.map(|received| KeyValue::new("Received", received)),
            sent.map(|sent| KeyValue::new("Sent", sent)),
        ]
        .into_iter()
        .flatten()
    });
    TooltipBody::dictionary(
        [KeyValue::new(tunnel.name.as_str(), "client")]
            .into_iter()
            .chain(activity_entry(tunnel))
            .chain(peer_entries)
            .chain(statistics_entry(&tunnel.statistics_access)),
    )
}

fn undetermined_tooltip(tunnel: &Tunnel) -> TooltipBody {
    TooltipBody::dictionary(
        [KeyValue::new(tunnel.name.as_str(), "tunnel")]
            .into_iter()
            .chain(statistics_entry(&tunnel.statistics_access)),
    )
}

fn client_state(tunnel: &Tunnel) -> WidgetState {
    match tunnel.statistics_access {
        StatisticsAccess::Read if tunnel.peers.iter().any(Peer::connected) => WidgetState::On,
        StatisticsAccess::Read => WidgetState::Pending,
        StatisticsAccess::TunnelDown
        | StatisticsAccess::NeedsNetAdmin
        | StatisticsAccess::Unreadable(_) => WidgetState::On,
    }
}

fn server_state(tunnel: &Tunnel) -> WidgetState {
    match tunnel.statistics_access {
        StatisticsAccess::Read if tunnel.connected_peers() > 0 => WidgetState::On,
        StatisticsAccess::Read => WidgetState::Idle,
        StatisticsAccess::TunnelDown
        | StatisticsAccess::NeedsNetAdmin
        | StatisticsAccess::Unreadable(_) => WidgetState::On,
    }
}

impl BarWidget for Tunnel {
    fn name(&self) -> &'static str {
        "WireGuard tunnel"
    }

    fn icon(&self) -> Glyph {
        match self.role {
            TunnelRole::Server { .. } => Phosphor::ArrowsInSimple(Weight::Regular).into(),
            TunnelRole::Client => Phosphor::MapPinLine(Weight::Regular).into(),
            TunnelRole::Undetermined => SimpleIcon::WireGuard.into(),
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        if self.switching() {
            return WidgetState::Pending;
        }
        if !self.up() {
            return WidgetState::Off;
        }
        match self.role {
            TunnelRole::Server { .. } => server_state(self),
            TunnelRole::Client => client_state(self),
            TunnelRole::Undetermined => WidgetState::On,
        }
    }

    fn badge(&self) -> Option<u32> {
        matches!(self.role, TunnelRole::Server { .. })
            .then(|| u32::try_from(self.connected_peers()).ok())
            .flatten()
            .filter(|count| *count > 0)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        match self.role {
            TunnelRole::Server { listen_port } => server_tooltip(self, listen_port),
            TunnelRole::Client => client_tooltip(self),
            TunnelRole::Undetermined => undetermined_tooltip(self),
        }
    }
}

impl ReadingSections for TunnelAddress {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary(
            self.ip.as_deref().map(|ip| KeyValue::new("IP", ip)),
        )]
    }
}
