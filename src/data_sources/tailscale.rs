//! Tailscale mesh status via `tailscale status --json`. A circle-nodes icon
//! with an online-peer badge when the backend is running; a dimmed icon when
//! not. Off the bar when `tailscale` is not installed.

use std::time::Duration;

use serde_json::Value as Json;

use crate::{
    data_sources::{
        BarWidget, Source, TooltipBody,
        command::{output, run},
        row,
    },
    widgets::icon::Icon,
};

#[derive(Debug, Clone)]
struct Peer {
    name: String,
    ip: String,
}

#[derive(Debug, Clone)]
pub struct Tailscale {
    running: bool,
    self_name: String,
    self_ip: String,
    online: Vec<Peer>,
    total_peers: usize,
}

/// The first label of a tailscale DNS name (`ryzen.tail….ts.net.` → `ryzen`).
fn short_name(dns: &str) -> String {
    dns.split('.').next().unwrap_or(dns).to_string()
}

fn first_ip(node: &Json) -> String {
    node.get("TailscaleIPs")
        .and_then(Json::as_array)
        .and_then(|ips| ips.first())
        .and_then(Json::as_str)
        .unwrap_or("—")
        .to_string()
}

impl BarWidget for Tailscale {
    fn icon(&self) -> Icon {
        Icon::CircleNodes
    }

    fn disabled(&self) -> bool {
        !self.running
    }

    fn badge(&self) -> Option<u32> {
        if !self.running {
            return None;
        }
        u32::try_from(self.online.len()).ok().filter(|c| *c > 0)
    }

    fn tooltip(&self) -> TooltipBody {
        if !self.running {
            return TooltipBody::Table(vec![row("Tailscale", "disconnected")]);
        }
        let mut rows = vec![
            row("Tailscale", "connected"),
            row("Self", self.self_name.clone()),
            row("IP", self.self_ip.clone()),
            row(
                "Peers",
                format!("{}/{} online", self.online.len(), self.total_peers),
            ),
        ];
        for peer in &self.online {
            rows.push(row(&peer.name, format!("→ {}", peer.ip)));
        }
        TooltipBody::Table(rows)
    }
}

impl Source for Tailscale {
    const NAME: &'static str = "tailscale";
    const INTERVAL: Duration = Duration::from_secs(10);

    fn poll() -> Option<Self> {
        let status: Json =
            serde_json::from_str(&output("tailscale", &["status", "--json"])?).ok()?;
        let running = status.get("BackendState").and_then(Json::as_str) == Some("Running");
        if !running {
            return Some(Tailscale {
                running: false,
                self_name: String::new(),
                self_ip: String::new(),
                online: Vec::new(),
                total_peers: 0,
            });
        }
        let self_node = status.get("Self");
        let self_name = self_node
            .and_then(|n| n.get("DNSName"))
            .and_then(Json::as_str)
            .map_or_else(|| "self".to_string(), short_name);
        let self_ip = self_node.map_or_else(|| "—".to_string(), first_ip);

        let peers = status.get("Peer").and_then(Json::as_object);
        let total_peers = peers.map_or(0, serde_json::Map::len);
        let online = peers
            .into_iter()
            .flatten()
            .filter(|(_, peer)| peer.get("Online").and_then(Json::as_bool) == Some(true))
            .map(|(_, peer)| Peer {
                name: peer
                    .get("DNSName")
                    .and_then(Json::as_str)
                    .map_or_else(|| "peer".to_string(), short_name),
                ip: first_ip(peer),
            })
            .collect();
        Some(Tailscale {
            running: true,
            self_name,
            self_ip,
            online,
            total_peers,
        })
    }
}

/// Open the tailscale admin console (odin's left-click on the tailscale pill).
pub fn open_admin() -> bool {
    run("xdg-open", &["https://login.tailscale.com/admin/machines"])
}
