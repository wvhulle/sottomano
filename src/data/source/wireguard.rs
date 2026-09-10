use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value as Json;

use crate::data::source::{Source, command, command::blocking::run};

const CONNECTED_WINDOW: u64 = 180;

#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub endpoint: String,
    pub allowed_ips: String,
    pub handshake_ago: Option<u64>,
    pub rx: u64,
    pub tx: u64,
}

impl Peer {
    pub fn connected(&self) -> bool {
        self.handshake_ago
            .is_some_and(|ago| ago <= CONNECTED_WINDOW)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tunnel {
    pub name: String,
    pub server: bool,
    pub ip: String,
    pub port: Option<u32>,
    pub peers: Vec<Peer>,
}

impl Tunnel {
    pub fn connected_peers(&self) -> usize {
        self.peers.iter().filter(|p| p.connected()).count()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wireguard {
    pub tunnels: Vec<Tunnel>,
}

impl Source for Wireguard {
    type Error = command::Error;

    type Key = ();

    const NAME: &'static str = "wireguard";
    const INTERVAL: Duration = Duration::from_secs(10);

    async fn available(_key: &()) -> bool {
        command::exists("wg").await
    }

    async fn poll(_key: ()) -> Result<Self, Self::Error> {
        let interfaces = command::require("wg", &["show", "interfaces"]).await?;
        let mut tunnels = Vec::new();
        for iface in interfaces.split_whitespace() {
            tunnels.push(read_tunnel(iface).await?);
        }
        Ok(Wireguard { tunnels })
    }
}

async fn read_tunnel(iface: &str) -> Result<Tunnel, command::Error> {
    let dump = command::require("wg", &["show", iface, "dump"]).await?;
    let mut lines = dump.lines();
    let interface = lines.next().ok_or_else(|| command::Error::Failed {
        program: "wg".to_owned(),
        stderr: format!("empty dump for {iface}"),
    })?;
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
    Ok(Tunnel {
        name: iface.to_string(),
        server: port.is_some(),
        ip: interface_ip(iface).await,
        port,
        peers,
    })
}

async fn interface_ip(iface: &str) -> String {
    command::output("ip", &["-j", "-4", "addr", "show", iface])
        .await
        .ok()
        .flatten()
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
