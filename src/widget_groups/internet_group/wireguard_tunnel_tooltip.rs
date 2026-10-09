use crate::{
    data::source::wireguard::{Peer, Tunnel, Wireguard},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Section, TooltipBody},
        widget_state::WidgetState,
    },
};

impl BarWidget for Wireguard {
    fn icon(&self) -> Glyph {
        Phosphor::ShieldCheck(self.state().weight()).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[])
    }
}

impl BarWidget for Tunnel {
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
        let connected_peers: Vec<(&str, &String)> = self
            .peers
            .iter()
            .filter(|peer| peer.connected())
            .map(|peer| {
                let key = if peer.endpoint.is_empty() {
                    "peer"
                } else {
                    peer.endpoint.as_str()
                };
                (key, &peer.allowed_ips)
            })
            .collect();
        TooltipBody::sections(&[
            Section::untitled(&[
                (self.name.as_str(), Some("server".to_owned())),
                ("IP", Some(self.ip.clone())),
                ("port", self.port.map(|port| port.to_string())),
                (
                    "clients",
                    Some(format!("{}/{}", self.connected_peers(), self.peers.len())),
                ),
            ]),
            Section::untitled(&connected_peers),
        ])
    }

    fn client_tooltip(&self) -> TooltipBody {
        let peer = self.peers.first();
        TooltipBody::sections(&[Section::untitled(&[
            (self.name.as_str(), Some("client".to_owned())),
            (
                "server",
                peer.map(|peer| peer.endpoint.clone())
                    .filter(|endpoint| !endpoint.is_empty()),
            ),
            ("IP", peer.map(|_| self.ip.clone())),
            (
                "handshake",
                peer.map(|peer| {
                    peer.handshake_ago
                        .map_or_else(|| "never".to_string(), |ago| format!("{ago}s ago"))
                }),
            ),
            (
                "received",
                peer.map(|peer| peer.rx.display().iec().to_string()),
            ),
            ("sent", peer.map(|peer| peer.tx.display().iec().to_string())),
        ])])
    }
}
