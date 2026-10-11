use crate::{
    data::source::wireguard::{Peer, Tunnel, Wireguard},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Wireguard {
    fn name(&self) -> &'static str {
        "WireGuard"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ShieldCheck(self.state().weight()).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[])
    }
}

impl BarWidget for Tunnel {
    fn name(&self) -> &'static str {
        "WireGuard tunnel"
    }

    fn icon(&self) -> Glyph {
        if self.server {
            Phosphor::ArrowsInSimple(self.state().weight()).into()
        } else {
            Phosphor::MapPinLine(self.state().weight()).into()
        }
    }

    fn state(&self) -> WidgetState {
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

    fn tooltip(&self) -> TooltipBody {
        if self.server {
            self.server_tooltip()
        } else {
            self.client_tooltip()
        }
    }
}

impl Tunnel {
    fn server_tooltip(&self) -> TooltipBody {
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
            self.ip.as_deref().map(|ip| KeyValue::new("IP", ip)),
            self.port
                .map(|port| KeyValue::new("port", port.to_string())),
            Some(KeyValue::new(
                "clients",
                format!("{}/{}", self.connected_peers(), self.peers.len()),
            )),
        ];
        TooltipBody::sections(&[
            Section::untitled_dictionary(tunnel_entries.into_iter().flatten()),
            Section::untitled_dictionary(connected_peers),
        ])
    }

    fn client_tooltip(&self) -> TooltipBody {
        let peer_entries = self.peers.first().into_iter().flat_map(|peer| {
            [
                peer.endpoint
                    .as_deref()
                    .map(|endpoint| KeyValue::new("server", endpoint)),
                self.ip.as_deref().map(|ip| KeyValue::new("IP", ip)),
                peer.handshake_ago
                    .map(|ago| KeyValue::new("handshake", format!("{ago}s ago"))),
                Some(KeyValue::new(
                    "received",
                    peer.rx.display().iec().to_string(),
                )),
                Some(KeyValue::new("sent", peer.tx.display().iec().to_string())),
            ]
            .into_iter()
            .flatten()
        });
        TooltipBody::sections(&[Section::untitled_dictionary(
            [KeyValue::new(self.name.as_str(), "client")]
                .into_iter()
                .chain(peer_entries),
        )])
    }
}
