use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value as Json;

use crate::data::source::{
    BarWidget, Glyph, Source, TooltipBody,
    command::{output, run},
    font_awesome::Solid,
    row,
};

const CONNECTED_WINDOW: u64 = 180;

#[derive(Debug, Clone, PartialEq)]
struct Peer {
    endpoint: String,
    allowed_ips: String,
    handshake_ago: Option<u64>,
    rx: u64,
    tx: u64,
}

impl Peer {
    fn connected(&self) -> bool {
        self.handshake_ago
            .is_some_and(|ago| ago <= CONNECTED_WINDOW)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tunnel {
    pub name: String,
    server: bool,
    ip: String,
    port: Option<u32>,
    peers: Vec<Peer>,
}

impl Tunnel {
    fn connected_peers(&self) -> usize {
        self.peers.iter().filter(|p| p.connected()).count()
    }
}

impl BarWidget for Tunnel {
    fn icon(&self) -> Glyph {
        if self.server {
            Solid::Minimize.into()
        } else {
            Solid::LocationPinLock.into()
        }
    }

    fn disabled(&self) -> bool {
        if self.server {
            self.connected_peers() == 0
        } else {
            !self.peers.iter().any(Peer::connected)
        }
    }

    fn badge(&self) -> Option<u32> {
        self.server
            .then(|| u32::try_from(self.connected_peers()).ok())
            .flatten()
            .filter(|c| *c > 0)
    }

    fn tooltip(&self) -> TooltipBody {
        let mut rows = Vec::new();
        if self.server {
            rows.push(row(&self.name, "server"));
            rows.push(row("IP", self.ip.clone()));
            if let Some(port) = self.port {
                rows.push(row("port", port.to_string()));
            }
            rows.push(row(
                "clients",
                format!("{}/{}", self.connected_peers(), self.peers.len()),
            ));
            for peer in self.peers.iter().filter(|p| p.connected()) {
                let key = if peer.endpoint.is_empty() {
                    "peer"
                } else {
                    &peer.endpoint
                };
                rows.push(row(key, peer.allowed_ips.clone()));
            }
        } else {
            rows.push(row(&self.name, "client"));
            if let Some(peer) = self.peers.first() {
                if !peer.endpoint.is_empty() {
                    rows.push(row("server", peer.endpoint.clone()));
                }
                rows.push(row("IP", self.ip.clone()));
                rows.push(row(
                    "handshake",
                    peer.handshake_ago
                        .map_or_else(|| "never".to_string(), |ago| format!("{ago}s ago")),
                ));
                rows.push(row(
                    "transfer",
                    format!("↓{} ↑{}", human_bytes(peer.rx), human_bytes(peer.tx)),
                ));
            }
        }
        TooltipBody::Table(rows)
    }
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wireguard {
    pub tunnels: Vec<Tunnel>,
}

impl BarWidget for Wireguard {
    fn icon(&self) -> Glyph {
        Solid::ShieldHalved.into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(Vec::new())
    }
}

impl Source for Wireguard {
    const NAME: &'static str = "wireguard";
    const INTERVAL: Duration = Duration::from_secs(10);

    fn poll() -> Option<Self> {
        let interfaces = output("wg", &["show", "interfaces"])?;
        let tunnels: Vec<Tunnel> = interfaces
            .split_whitespace()
            .filter_map(read_tunnel)
            .collect();
        (!tunnels.is_empty()).then_some(Wireguard { tunnels })
    }
}

fn read_tunnel(iface: &str) -> Option<Tunnel> {
    let dump = output("wg", &["show", iface, "dump"])?;
    let mut lines = dump.lines();
    let interface = lines.next()?;
    let port: Option<u32> = interface
        .split('\t')
        .nth(2)
        .and_then(|p| p.parse().ok())
        .filter(|p| *p > 0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let peers = lines
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let handshake: u64 = f.get(4).and_then(|h| h.parse().ok()).unwrap_or(0);
            Peer {
                endpoint: f
                    .get(2)
                    .filter(|e| **e != "(none)")
                    .unwrap_or(&"")
                    .to_string(),
                allowed_ips: f.get(3).unwrap_or(&"—").to_string(),
                handshake_ago: (handshake > 0).then(|| now.saturating_sub(handshake)),
                rx: f.get(5).and_then(|v| v.parse().ok()).unwrap_or(0),
                tx: f.get(6).and_then(|v| v.parse().ok()).unwrap_or(0),
            }
        })
        .collect();
    Some(Tunnel {
        name: iface.to_string(),
        server: port.is_some(),
        ip: interface_ip(iface),
        port,
        peers,
    })
}

fn interface_ip(iface: &str) -> String {
    output("ip", &["-j", "-4", "addr", "show", iface])
        .and_then(|out| serde_json::from_str::<Json>(&out).ok())
        .and_then(|json| {
            let entry = json.as_array()?.first()?;
            let addr = entry.get("addr_info")?.as_array()?.first()?;
            let local = addr.get("local")?.as_str()?;
            let prefix = addr.get("prefixlen").and_then(Json::as_u64).unwrap_or(32);
            Some(format!("{local}/{prefix}"))
        })
        .unwrap_or_else(|| "—".to_string())
}

pub fn disconnect(name: &str) -> bool {
    run("nmcli", &["connection", "down", name])
}
