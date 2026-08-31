use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    time::Duration,
};

use serde_json::Value as Json;

use crate::data::source::{
    BarWidget, Glyph, Source, TooltipBody, command, command::blocking::run, font_awesome::Solid,
    row,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Command(command::Error),
    Malformed,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Command(error) => write!(f, "{error}"),
            Error::Malformed => write!(f, "unexpected status JSON"),
        }
    }
}

impl From<command::Error> for Error {
    fn from(error: command::Error) -> Self {
        Error::Command(error)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Peer {
    name: String,
    ip: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tailscale {
    running: bool,
    self_name: String,
    self_ip: String,
    online: Vec<Peer>,
    total_peers: usize,
}

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
    fn icon(&self) -> Glyph {
        Solid::CircleNodes.into()
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
            return TooltipBody::rows(vec![row("Tailscale", "disconnected")]);
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
        TooltipBody::rows(rows)
    }
}

impl Source for Tailscale {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "tailscale";
    const INTERVAL: Duration = Duration::from_secs(10);

    async fn available(_key: &()) -> bool {
        command::exists("tailscale").await
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        let raw = command::require("tailscale", &["status", "--json"]).await?;
        let status: Json = serde_json::from_str(&raw).map_err(|_| Error::Malformed)?;
        let running = status.get("BackendState").and_then(Json::as_str) == Some("Running");
        if !running {
            return Ok(Tailscale {
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
        Ok(Tailscale {
            running: true,
            self_name,
            self_ip,
            online,
            total_peers,
        })
    }
}

pub fn open_admin() -> bool {
    run("xdg-open", &["https://login.tailscale.com/admin/machines"])
}
