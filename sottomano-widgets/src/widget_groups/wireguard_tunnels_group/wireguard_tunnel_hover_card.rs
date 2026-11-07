use std::borrow::Cow;

use iced::alignment::Horizontal;
use sottomano_desktop_sources::network_connectivity::wireguard_tunnels::{
    Peer, PeerEndpoint, PeerTraffic, StatisticsAccess, Tunnel, TunnelAddress, TunnelRole,
    Wireguard, WireguardSwitch,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem, group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState, toggle_control::SwitchPosition,
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, ReadingSections, Section},
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        simple_icons::SimpleIcon,
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(super) fn wireguard_tunnel_item<Message: Clone + 'static>(
    tunnel: &Tunnel,
    target: impl Into<HoverTarget>,
    on_switch: fn(WireguardSwitch) -> Message,
) -> GroupItem<'_, Message> {
    let item = GroupItem::reading(tunnel, target);
    match tunnel.switch() {
        Some(wireguard_switch @ WireguardSwitch::Connect(_)) => item.on_switch_press(
            SwitchPosition::Off,
            "connect the tunnel",
            on_switch(wireguard_switch),
        ),
        Some(wireguard_switch @ WireguardSwitch::Disconnect(_)) => item.on_switch_press(
            SwitchPosition::On,
            "disconnect the tunnel",
            on_switch(wireguard_switch),
        ),
        None => item,
    }
}

impl GroupItemReading for Wireguard {
    fn name(&self) -> &'static str {
        "WireGuard"
    }

    fn icon(&self) -> IconGlyph {
        SimpleIcon::WireGuard.into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([])
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

impl IntoTableRecord<4> for &Peer {
    fn into_table_record(self) -> TableRecord<4> {
        let [received, sent] =
            transfer_cells(self.traffic).unwrap_or([ValueCell::Blank, ValueCell::Blank]);
        TableRecord::new(
            peer_label(self),
            [
                endpoint_cell(&self.endpoint).unwrap_or(ValueCell::Blank),
                handshake_cell(self.traffic).unwrap_or(ValueCell::Blank),
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
    if tunnel.switching() {
        Some(LabeledValue::new("State", "switching"))
    } else if tunnel.up() {
        None
    } else {
        Some(LabeledValue::new("State", "not connected"))
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

fn server_hover_card(tunnel: &Tunnel, listen_port: u32) -> HoverCardBody {
    let summary_section = Section::dictionary(
        [
            LabeledValue::new(tunnel.name.as_str(), "server"),
            LabeledValue::new("Listen port", listen_port.to_string()),
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
    HoverCardBody::sections([summary_section, peer_table])
}

fn client_hover_card(tunnel: &Tunnel) -> HoverCardBody {
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
    HoverCardBody::dictionary(
        [LabeledValue::new(tunnel.name.as_str(), "client")]
            .into_iter()
            .chain(activity_entry(tunnel))
            .chain(peer_entries)
            .chain(statistics_entry(&tunnel.statistics_access)),
    )
}

fn undetermined_hover_card(tunnel: &Tunnel) -> HoverCardBody {
    HoverCardBody::dictionary(
        [LabeledValue::new(tunnel.name.as_str(), "tunnel")]
            .into_iter()
            .chain(statistics_entry(&tunnel.statistics_access)),
    )
}

fn client_state(tunnel: &Tunnel) -> ReadingDisplayState {
    match tunnel.statistics_access {
        StatisticsAccess::Read if tunnel.peers.iter().any(Peer::connected) => {
            ReadingDisplayState::On
        }
        StatisticsAccess::Read => ReadingDisplayState::Pending,
        StatisticsAccess::TunnelDown
        | StatisticsAccess::NeedsNetAdmin
        | StatisticsAccess::Unreadable(_) => ReadingDisplayState::On,
    }
}

fn server_state(tunnel: &Tunnel) -> ReadingDisplayState {
    match tunnel.statistics_access {
        StatisticsAccess::Read if tunnel.connected_peers() > 0 => ReadingDisplayState::On,
        StatisticsAccess::Read => ReadingDisplayState::Idle,
        StatisticsAccess::TunnelDown
        | StatisticsAccess::NeedsNetAdmin
        | StatisticsAccess::Unreadable(_) => ReadingDisplayState::On,
    }
}

impl GroupItemReading for Tunnel {
    fn name(&self) -> &'static str {
        "WireGuard tunnel"
    }

    fn menu_row_name(&self) -> Cow<'static, str> {
        self.name.clone().into()
    }

    fn icon(&self) -> IconGlyph {
        match self.role {
            TunnelRole::Server { .. } => Phosphor::HardDrives(Weight::Regular).into(),
            TunnelRole::Client if self.peers.iter().any(Peer::routes_all_traffic) => {
                Phosphor::GlobeHemisphereWest(Weight::Regular).into()
            }
            TunnelRole::Client => Phosphor::PlugsConnected(Weight::Regular).into(),
            TunnelRole::Undetermined => SimpleIcon::WireGuard.into(),
        }
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        if self.switching() {
            return ReadingDisplayState::Pending;
        }
        if !self.up() {
            return ReadingDisplayState::Off;
        }
        match self.role {
            TunnelRole::Server { .. } => server_state(self),
            TunnelRole::Client => client_state(self),
            TunnelRole::Undetermined => ReadingDisplayState::On,
        }
    }

    fn badge(&self) -> Option<u32> {
        matches!(self.role, TunnelRole::Server { .. })
            .then(|| u32::try_from(self.connected_peers()).ok())
            .flatten()
            .filter(|count| *count > 0)
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        match self.role {
            TunnelRole::Server { listen_port } => server_hover_card(self, listen_port),
            TunnelRole::Client => client_hover_card(self),
            TunnelRole::Undetermined => undetermined_hover_card(self),
        }
    }
}

impl ReadingSections for TunnelAddress {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary(
            self.ip.as_deref().map(|ip| LabeledValue::new("IP", ip)),
        )]
    }
}
