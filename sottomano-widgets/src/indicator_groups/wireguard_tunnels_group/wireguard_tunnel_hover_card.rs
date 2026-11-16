use iced::alignment::Horizontal;
use sottomano_desktop_sources::network_connectivity::{
    network_manager_proxies::{LinkPhase, NmConnectionSwitch},
    wireguard_tunnels::{
        Peer, PeerEndpoint, PeerLink, PeerTraffic, RouteScope, StatisticsAccess, Tunnel,
        TunnelAddress, TunnelRole,
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::SwitchPosition,
    },
    hover_cards::hover_card_body::{Section, dictionary_table, headed_table},
    indicator_groups::indicator_group_interaction::IndicatorGroupInteraction,
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(super) fn wireguard_tunnel_indicator<Message: Into<IndicatorGroupInteraction>>(
    tunnel: &Tunnel,
    on_switch: fn(NmConnectionSwitch) -> Message,
) -> GroupIndicator<'_> {
    let indicator = tunnel_indicator(tunnel);
    match tunnel.switch() {
        Some(wireguard_switch @ NmConnectionSwitch::Connect(_)) => indicator.on_switch_press(
            SwitchPosition::Off,
            "connect the tunnel",
            on_switch(wireguard_switch),
        ),
        Some(wireguard_switch @ NmConnectionSwitch::Disconnect(_)) => indicator.on_switch_press(
            SwitchPosition::On,
            "disconnect the tunnel",
            on_switch(wireguard_switch),
        ),
        None => indicator,
    }
}

fn endpoint_cell(endpoint: &PeerEndpoint) -> Option<ValueCell> {
    match endpoint {
        PeerEndpoint::At(address) => Some(address.as_str().into()),
        PeerEndpoint::Roaming => None,
    }
}

fn handshake_cell(traffic: PeerTraffic) -> Option<ValueCell> {
    match traffic {
        PeerTraffic::Exchanged(statistics) => Some(ValueCell::duration(statistics.handshake_ago)),
        PeerTraffic::NeverHandshaken => Some("never".into()),
        PeerTraffic::Unobserved => None,
    }
}

fn transfer_cells(traffic: PeerTraffic) -> Option<[ValueCell; 2]> {
    match traffic {
        PeerTraffic::Exchanged(statistics) => Some([
            Quantity::size(statistics.received).into(),
            Quantity::size(statistics.sent).into(),
        ]),
        PeerTraffic::NeverHandshaken | PeerTraffic::Unobserved => None,
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

impl From<&Peer> for TableRecord<4> {
    fn from(peer: &Peer) -> Self {
        let [received, sent] =
            transfer_cells(peer.traffic).unwrap_or([ValueCell::Blank, ValueCell::Blank]);
        TableRecord::with_cells(
            peer_label(peer),
            [
                endpoint_cell(&peer.endpoint).unwrap_or(ValueCell::Blank),
                handshake_cell(peer.traffic).unwrap_or(ValueCell::Blank),
                received,
                sent,
            ],
        )
    }
}

fn statistics_entry(statistics_access: &StatisticsAccess) -> Option<LabeledValue> {
    match statistics_access {
        StatisticsAccess::NeedsNetAdmin => {
            Some(LabeledValue::new("Peer stats", "need CAP_NET_ADMIN"))
        }
        StatisticsAccess::Unreadable(detail) => {
            Some(LabeledValue::new("Peer stats", detail.as_str()).level(Severity::Warning))
        }
        StatisticsAccess::TunnelDown | StatisticsAccess::Read => None,
    }
}

fn activity_entry(tunnel: &Tunnel) -> Option<LabeledValue> {
    match tunnel.tunnel_phase() {
        LinkPhase::Switching => Some(LabeledValue::new("State", "switching")),
        LinkPhase::Up => None,
        LinkPhase::Down => Some(LabeledValue::new("State", "not connected")),
    }
}

fn clients_entry(tunnel: &Tunnel) -> LabeledValue {
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
    LabeledValue::new("Clients", clients)
}

fn server_hover_card(tunnel: &Tunnel, listen_port: u32) -> Vec<Section> {
    let summary_section = Section::new(
        "Status",
        dictionary_table(
            [
                LabeledValue::new(tunnel.name.as_str(), "server"),
                LabeledValue::new("Listen port", listen_port.to_string()),
                clients_entry(tunnel),
            ]
            .into_iter()
            .chain(activity_entry(tunnel))
            .chain(statistics_entry(&tunnel.statistics_access)),
        ),
    );
    let peer_table = Section::new(
        "Clients",
        headed_table(
            [
                ColumnHeading::titled("Endpoint").align_x(Horizontal::Left),
                ColumnHeading::titled("Handshake"),
                ColumnHeading::titled("Received"),
                ColumnHeading::titled("Sent"),
            ],
            &tunnel.peers,
        ),
    );
    vec![summary_section, peer_table]
}

fn client_hover_card(tunnel: &Tunnel) -> Vec<Section> {
    let peer_entries = tunnel.peers.iter().flat_map(|peer| {
        let transfer_entries =
            transfer_cells(peer.traffic)
                .into_iter()
                .flat_map(|[received, sent]| {
                    [
                        LabeledValue::new("Received", received),
                        LabeledValue::new("Sent", sent),
                    ]
                });
        [
            endpoint_cell(&peer.endpoint).map(|endpoint| LabeledValue::new("Server", endpoint)),
            Some(LabeledValue::new("Allowed IPs", allowed_ips_text(peer))),
            handshake_cell(peer.traffic).map(|handshake| LabeledValue::new("Handshake", handshake)),
        ]
        .into_iter()
        .flatten()
        .chain(transfer_entries)
    });
    vec![Section::new(
        "Status",
        dictionary_table(
            [LabeledValue::new(tunnel.name.as_str(), "client")]
                .into_iter()
                .chain(activity_entry(tunnel))
                .chain(peer_entries)
                .chain(statistics_entry(&tunnel.statistics_access)),
        ),
    )]
}

fn undetermined_hover_card(tunnel: &Tunnel) -> Vec<Section> {
    vec![Section::new(
        "Status",
        dictionary_table(
            [LabeledValue::new(tunnel.name.as_str(), "tunnel")]
                .into_iter()
                .chain(statistics_entry(&tunnel.statistics_access)),
        ),
    )]
}

fn client_state(tunnel: &Tunnel) -> IndicatorState {
    match tunnel.statistics_access {
        StatisticsAccess::Read
            if tunnel
                .peers
                .iter()
                .any(|peer| peer.traffic.peer_link() == PeerLink::Connected) =>
        {
            IndicatorState::On
        }
        StatisticsAccess::Read => IndicatorState::Pending,
        StatisticsAccess::TunnelDown
        | StatisticsAccess::NeedsNetAdmin
        | StatisticsAccess::Unreadable(_) => IndicatorState::On,
    }
}

fn server_state(tunnel: &Tunnel) -> IndicatorState {
    match tunnel.statistics_access {
        StatisticsAccess::Read if tunnel.connected_peers() > 0 => IndicatorState::On,
        StatisticsAccess::Read => IndicatorState::Idle,
        StatisticsAccess::TunnelDown
        | StatisticsAccess::NeedsNetAdmin
        | StatisticsAccess::Unreadable(_) => IndicatorState::On,
    }
}

pub(super) fn tunnel_indicator(tunnel: &Tunnel) -> GroupIndicator<'_> {
    let glyph: IconGlyph = match tunnel.role {
        TunnelRole::Server { .. } => Phosphor::HardDrives(Weight::Regular).into(),
        TunnelRole::Client
            if tunnel
                .peers
                .iter()
                .any(|peer| peer.route_scope() == RouteScope::AllTraffic) =>
        {
            Phosphor::GlobeHemisphereWest(Weight::Regular).into()
        }
        TunnelRole::Client => Phosphor::PlugsConnected(Weight::Regular).into(),
        TunnelRole::Undetermined => Phosphor::Path(Weight::Regular).into(),
    };
    let state = match tunnel.tunnel_phase() {
        LinkPhase::Switching => IndicatorState::Pending,
        LinkPhase::Down => IndicatorState::Off,
        LinkPhase::Up => match tunnel.role {
            TunnelRole::Server { .. } => server_state(tunnel),
            TunnelRole::Client => client_state(tunnel),
            TunnelRole::Undetermined => IndicatorState::On,
        },
    };
    let connected_server_peers = match tunnel.role {
        TunnelRole::Server { .. } => tunnel.connected_peers(),
        TunnelRole::Client | TunnelRole::Undetermined => 0,
    };
    let badge = match connected_server_peers {
        0 => ReadoutBadge::Unbadged,
        count => ReadoutBadge::Counted(count),
    };
    GroupIndicator::glyph_only(tunnel.name.clone(), glyph, state)
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Ungauged,
            badge,
        })
        .with_hover_card(Box::new(move |_| tunnel_hover_card(tunnel)))
}

pub(super) fn tunnel_hover_card(tunnel: &Tunnel) -> Vec<Section> {
    match tunnel.role {
        TunnelRole::Server { listen_port } => server_hover_card(tunnel, listen_port),
        TunnelRole::Client => client_hover_card(tunnel),
        TunnelRole::Undetermined => undetermined_hover_card(tunnel),
    }
}

pub fn tunnel_address_sections(tunnel_address: &TunnelAddress) -> Vec<Section> {
    vec![Section::new(
        "Addresses",
        dictionary_table(
            tunnel_address
                .ip
                .as_deref()
                .map(|ip| LabeledValue::new("IP", ip)),
        ),
    )]
}
