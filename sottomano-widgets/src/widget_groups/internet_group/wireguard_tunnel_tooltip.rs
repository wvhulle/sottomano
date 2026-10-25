use std::time::Duration;

use sottomano_sources::network_connectivity::wireguard_tunnels::{
    Peer, Tunnel, TunnelAddress, Wireguard,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, ReadingSections, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Wireguard {
    fn name(&self) -> &'static str {
        "WireGuard"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ShieldCheck(Weight::Regular).into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([])
    }
}

impl BarWidget for Tunnel {
    fn name(&self) -> &'static str {
        "WireGuard tunnel"
    }

    fn icon(&self) -> Glyph {
        if self.server {
            Phosphor::ArrowsInSimple(Weight::Regular).into()
        } else {
            Phosphor::MapPinLine(Weight::Regular).into()
        }
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        WidgetState::of_toggle(if self.server {
            self.connected_peers() > 0
        } else {
            self.peers.iter().any(Peer::connected)
        })
    }

    fn badge(&self) -> Option<u32> {
        self.server
            .then(|| u32::try_from(self.connected_peers()).ok())
            .flatten()
            .filter(|count| *count > 0)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        if !self.server {
            let peer_entries = self.peers.first().into_iter().flat_map(|peer| {
                [
                    peer.endpoint
                        .as_deref()
                        .map(|endpoint| KeyValue::new("server", endpoint)),
                    peer.handshake_ago.map(|ago| {
                        KeyValue::new("handshake ago", Cell::duration(Duration::from_secs(ago)))
                    }),
                    Some(KeyValue::new(
                        "received",
                        peer.rx.display().iec().to_string(),
                    )),
                    Some(KeyValue::new("sent", peer.tx.display().iec().to_string())),
                ]
                .into_iter()
                .flatten()
            });
            return TooltipBody::dictionary(
                [KeyValue::new(self.name.as_str(), "client")]
                    .into_iter()
                    .chain(peer_entries),
            );
        }
        let connected_peers = self
            .peers
            .iter()
            .filter(|peer| peer.connected())
            .filter_map(|peer| {
                let key = peer.endpoint.as_deref().unwrap_or("peer");
                Some(KeyValue::new(key, peer.allowed_ips.as_deref()?))
            });
        let tunnel_entries = [
            Some(KeyValue::new(self.name.as_str(), "server")),
            self.port
                .map(|port| KeyValue::new("port", port.to_string())),
            Some(KeyValue::new(
                "clients",
                format!("{}/{}", self.connected_peers(), self.peers.len()),
            )),
        ];
        TooltipBody::sections([
            Section::dictionary(tunnel_entries.into_iter().flatten()),
            Section::dictionary(connected_peers),
        ])
    }
}

impl ReadingSections for TunnelAddress {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary(
            self.ip.as_deref().map(|ip| KeyValue::new("IP", ip)),
        )]
    }
}
