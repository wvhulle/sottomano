use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytesize::ByteSize;
use futures::{Stream, StreamExt, stream};
use serde_json::Value as Json;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    network_connectivity::network_manager_proxies,
    polled_delivery_stream, pushed_delivery_stream,
    stream_operators::{StreamOperators, read_once_then_per_burst},
    system_protocol_clients::{
        dbus_desktop_buses::DesktopBuses, shell_command_output, shell_command_output::blocking::run,
    },
};

const CONNECTED_WINDOW: u64 = 180;

#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub endpoint: Option<String>,
    pub allowed_ips: Option<String>,
    pub handshake_ago: Option<u64>,
    pub rx: ByteSize,
    pub tx: ByteSize,
}

impl Peer {
    #[must_use]
    pub fn connected(&self) -> bool {
        self.handshake_ago
            .is_some_and(|ago| ago <= CONNECTED_WINDOW)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tunnel {
    pub name: String,
    pub server: bool,
    pub port: Option<u32>,
    pub peers: Vec<Peer>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TunnelAddress {
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TunnelOnBus {
    pub buses: DesktopBuses,
    pub tunnel: String,
}

impl ReadingSource for TunnelAddress {
    type Error = shell_command_output::Error;
    type StreamInputs = TunnelOnBus;

    const NAME: &'static str = "wireguard tunnel address";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(watched: TunnelOnBus) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        pushed_delivery_stream(address_changes(watched))
    }
}

async fn address_changes(
    watched: TunnelOnBus,
) -> Result<
    impl Stream<Item = Result<TunnelAddress, shell_command_output::Error>> + Send,
    shell_command_output::Error,
> {
    let addressing = match network_manager_proxies::changes(
        &watched.buses.system,
        ADDRESSING_PROPERTIES,
    )
    .await
    {
        Ok(signals) => signals.left_stream(),
        Err(e) => {
            tracing::debug!("wireguard: no NetworkManager to follow: {e}");
            stream::empty().right_stream()
        }
    };
    Ok(read_once_then_per_burst(addressing, move || {
        let tunnel = watched.tunnel.clone();
        async move {
            Ok(TunnelAddress {
                ip: interface_ip(&tunnel).await,
            })
        }
    }))
}

const ADDRESSING_PROPERTIES: &[&str] = &[
    network_manager_proxies::DEVICE_PROPERTIES,
    network_manager_proxies::IP4_CONFIG_PROPERTIES,
];

impl Tunnel {
    #[must_use]
    pub fn connected_peers(&self) -> usize {
        self.peers.iter().filter(|p| p.connected()).count()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wireguard {
    pub tunnels: Vec<Tunnel>,
}

impl ReadingSource for Wireguard {
    type Error = shell_command_output::Error;
    type StreamInputs = ();

    const NAME: &'static str = "wireguard";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(10));

    fn delivery_stream(inputs: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("wg")).while_held(
            move || polled_delivery_stream(inputs, Self::poll),
            SourceDelivery::Absent,
        )
    }
}

impl Wireguard {
    async fn poll(_key: ()) -> Result<Self, shell_command_output::Error> {
        let interfaces = shell_command_output::output("wg", &["show", "interfaces"]).await?;
        let mut tunnels = Vec::new();
        for iface in interfaces.split_whitespace() {
            tunnels.push(read_tunnel(iface).await?);
        }
        Ok(Wireguard { tunnels })
    }
}

async fn read_tunnel(iface: &str) -> Result<Tunnel, shell_command_output::Error> {
    let dump = shell_command_output::output("wg", &["show", iface, "dump"]).await?;
    let mut lines = dump.lines();
    let interface = lines
        .next()
        .ok_or_else(|| shell_command_output::Error::Failed {
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
    let peers = lines.filter_map(|line| peer(line, now)).collect();
    Ok(Tunnel {
        name: iface.to_string(),
        server: port.is_some(),
        port,
        peers,
    })
}

fn peer(dump_line: &str, now: u64) -> Option<Peer> {
    let fields: Vec<&str> = dump_line.split('\t').collect();
    let reported = |position: usize| {
        fields
            .get(position)
            .filter(|field| **field != "(none)")
            .map(ToString::to_string)
    };
    let handshake: u64 = fields.get(4)?.parse().ok()?;
    Some(Peer {
        endpoint: reported(2),
        allowed_ips: reported(3),
        handshake_ago: (handshake > 0).then(|| now.saturating_sub(handshake)),
        rx: ByteSize::b(fields.get(5)?.parse().ok()?),
        tx: ByteSize::b(fields.get(6)?.parse().ok()?),
    })
}

async fn interface_ip(iface: &str) -> Option<String> {
    shell_command_output::output("ip", &["-j", "-4", "addr", "show", iface])
        .await
        .ok()
        .and_then(|out| serde_json::from_str::<Json>(&out).ok())
        .and_then(|json| {
            let entry = json.as_array()?.first()?;
            let addr = entry.get("addr_info")?.as_array()?.first()?;
            let local = addr.get("local")?.as_str()?;
            let prefix = addr.get("prefixlen").and_then(Json::as_u64).unwrap_or(32);
            Some(format!("{local}/{prefix}"))
        })
}

#[must_use]
pub fn disconnect(name: &str) -> bool {
    run("nmcli", &["connection", "down", name])
}
