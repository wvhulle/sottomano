use crate::{
    data::source::wireguard::{Peer, Tunnel, Wireguard},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{Row, TooltipBody, row},
        widget_state::WidgetState,
    },
};

impl BarWidget for Wireguard {
    fn icon(&self) -> Glyph {
        Phosphor::ShieldCheck(self.state().weight()).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(Vec::new())
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
        TooltipBody::rows(if self.server {
            server_rows(self)
        } else {
            client_rows(self)
        })
    }
}

fn server_rows(tunnel: &Tunnel) -> Vec<Row> {
    let mut rows = vec![row(&tunnel.name, "server"), row("IP", tunnel.ip.clone())];
    rows.extend(tunnel.port.map(|port| row("port", port.to_string())));
    rows.push(row(
        "clients",
        format!("{}/{}", tunnel.connected_peers(), tunnel.peers.len()),
    ));
    rows.extend(
        tunnel
            .peers
            .iter()
            .filter(|peer| peer.connected())
            .map(|peer| {
                let key = if peer.endpoint.is_empty() {
                    "peer"
                } else {
                    &peer.endpoint
                };
                row(key, peer.allowed_ips.clone())
            }),
    );
    rows
}

fn client_rows(tunnel: &Tunnel) -> Vec<Row> {
    let mut rows = vec![row(&tunnel.name, "client")];
    let Some(peer) = tunnel.peers.first() else {
        return rows;
    };
    if !peer.endpoint.is_empty() {
        rows.push(row("server", peer.endpoint.clone()));
    }
    rows.push(row("IP", tunnel.ip.clone()));
    rows.push(row(
        "handshake",
        peer.handshake_ago
            .map_or_else(|| "never".to_string(), |ago| format!("{ago}s ago")),
    ));
    rows.push(row("received", peer.rx.display().iec().to_string()));
    rows.push(row("sent", peer.tx.display().iec().to_string()));
    rows
}
