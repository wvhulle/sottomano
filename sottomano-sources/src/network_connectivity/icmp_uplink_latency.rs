use std::{
    collections::VecDeque,
    fmt::{Display, Formatter, Result as FmtResult},
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket},
    pin::Pin,
    time::{Duration, Instant},
};

use futures::{
    Stream, StreamExt,
    future::join,
    stream::{self, select},
};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::{net::UdpSocket, time::timeout};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    SnapshotDelivery, SnapshotSource, SourceCaption,
    delivery_cadence_drivers::{PeriodicPolling, PollTick},
    network_connectivity::{network_manager_proxies, nm_readings},
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

const PROBE_POLLING: PeriodicPolling = PeriodicPolling::every(Duration::from_secs(5));
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const WINDOW_PROBES: usize = 20;
const ICMPV4_ECHO_REQUEST: u8 = 8;
const ICMPV4_ECHO_REPLY: u8 = 0;
const ICMPV6_ECHO_REQUEST: u8 = 128;
const ICMPV6_ECHO_REPLY: u8 = 129;
const ECHO_PAYLOAD: &[u8] = b"sottomano";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LatencyProbeTargets {
    pub buses: DesktopBuses,
    pub internet_host: IpAddr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOutcome {
    Answered(Duration),
    Lost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeWindow {
    pub host: IpAddr,
    pub outcomes: VecDeque<ProbeOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayProbe {
    Probed(ProbeWindow),
    NoGateway,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UplinkLatency {
    pub gateway: GatewayProbe,
    pub internet: ProbeWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LatencyProbeError {
    IcmpSocketDenied(String),
}

impl Display for LatencyProbeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            LatencyProbeError::IcmpSocketDenied(detail) => write!(
                formatter,
                "cannot open an unprivileged ICMP socket ({detail}); check \
                 net.ipv4.ping_group_range"
            ),
        }
    }
}

impl ProbeWindow {
    #[must_use]
    pub fn empty(host: IpAddr) -> Self {
        Self {
            host,
            outcomes: VecDeque::new(),
        }
    }

    #[must_use]
    pub fn recorded(mut self, outcome: ProbeOutcome) -> Self {
        self.outcomes.push_back(outcome);
        if self.outcomes.len() > WINDOW_PROBES {
            self.outcomes.pop_front();
        }
        self
    }

    fn answered_round_trips(&self) -> impl Iterator<Item = Duration> + '_ {
        self.outcomes.iter().filter_map(|outcome| match outcome {
            ProbeOutcome::Answered(round_trip) => Some(*round_trip),
            ProbeOutcome::Lost => None,
        })
    }

    #[must_use]
    pub fn latest_outcome(&self) -> Option<ProbeOutcome> {
        self.outcomes.back().copied()
    }

    #[must_use]
    pub fn median_round_trip(&self) -> Option<Duration> {
        let mut round_trips: Vec<Duration> = self.answered_round_trips().collect();
        round_trips.sort_unstable();
        round_trips.get(round_trips.len() / 2).copied()
    }

    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn loss_share(&self) -> Ratio {
        if self.outcomes.is_empty() {
            return Ratio::new::<ratio>(0.0);
        }
        let lost_count = self
            .outcomes
            .iter()
            .filter(|outcome| **outcome == ProbeOutcome::Lost)
            .count();
        Ratio::new::<ratio>(lost_count as f64 / self.outcomes.len() as f64)
    }
}

impl SnapshotSource for UplinkLatency {
    type Error = LatencyProbeError;
    type StreamInputs = LatencyProbeTargets;

    const NAME: &'static str = "Latency";
    const CAPTION: SourceCaption = SourceCaption::Captioned(
        "Echo round trips every five seconds to the router and to an internet host. A slow router \
         points at the local network; a slow internet host alone points at the provider.",
    );

    fn delivery_stream(
        targets: LatencyProbeTargets,
    ) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let gateway_changes = nm_readings(
            targets.buses.system,
            network_manager_proxies::UPLINK_PROPERTIES,
            network_manager_proxies::uplink,
        )
        .map(|uplink| match uplink {
            Ok(uplink) => ProbeEvent::GatewayChanged(uplink.and_then(|uplink| uplink.gateway)),
            Err(_) => ProbeEvent::GatewayChanged(None),
        });
        let ticks = PROBE_POLLING.poll_ticks().map(ProbeEvent::Tick);
        let probe_events: Pin<Box<dyn Stream<Item = ProbeEvent> + Send>> =
            Box::pin(select(gateway_changes, ticks));
        let prober = LatencyProber {
            gateway: GatewayProbe::NoGateway,
            internet: ProbeWindow::empty(targets.internet_host),
            next_sequence: 0,
        };
        stream::unfold(
            (probe_events, prober),
            |(mut probe_events, mut prober)| async move {
                loop {
                    match probe_events.next().await? {
                        ProbeEvent::GatewayChanged(gateway) => prober.retarget_gateway(gateway),
                        ProbeEvent::Tick(poll_tick) => {
                            let delivery = prober.probe_round(poll_tick).await;
                            return Some((delivery, (probe_events, prober)));
                        }
                    }
                }
            },
        )
        .distinct_until_changed()
    }
}

enum ProbeEvent {
    GatewayChanged(Option<Ipv4Addr>),
    Tick(PollTick),
}

struct LatencyProber {
    gateway: GatewayProbe,
    internet: ProbeWindow,
    next_sequence: u16,
}

impl LatencyProber {
    fn retarget_gateway(&mut self, gateway: Option<Ipv4Addr>) {
        let unchanged = match (&self.gateway, gateway) {
            (GatewayProbe::Probed(window), Some(address)) => window.host == IpAddr::V4(address),
            (GatewayProbe::NoGateway, None) => true,
            _ => false,
        };
        if !unchanged {
            self.gateway = gateway.map_or(GatewayProbe::NoGateway, |address| {
                GatewayProbe::Probed(ProbeWindow::empty(IpAddr::V4(address)))
            });
        }
    }

    async fn probe_round(&mut self, poll_tick: PollTick) -> SnapshotDelivery<UplinkLatency> {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let gateway_host = match &self.gateway {
            GatewayProbe::Probed(window) => Some(window.host),
            GatewayProbe::NoGateway => None,
        };
        let (internet_outcome, gateway_outcome) =
            join(echo_round_trip(self.internet.host, sequence), async {
                match gateway_host {
                    Some(host) => Some(echo_round_trip(host, sequence).await),
                    None => None,
                }
            })
            .await;
        let internet_outcome = match internet_outcome {
            Ok(outcome) => outcome,
            Err(failure) => return poll_tick.stamped(Err(failure)),
        };
        self.internet = self.internet.clone().recorded(internet_outcome);
        if let (GatewayProbe::Probed(window), Some(Ok(outcome))) = (&self.gateway, gateway_outcome)
        {
            self.gateway = GatewayProbe::Probed(window.clone().recorded(outcome));
        }
        poll_tick.stamped(Ok(UplinkLatency {
            gateway: self.gateway.clone(),
            internet: self.internet.clone(),
        }))
    }
}

async fn echo_round_trip(host: IpAddr, sequence: u16) -> Result<ProbeOutcome, LatencyProbeError> {
    let (domain, protocol, request_kind, reply_kind) = match host {
        IpAddr::V4(_) => (
            Domain::IPV4,
            Protocol::ICMPV4,
            ICMPV4_ECHO_REQUEST,
            ICMPV4_ECHO_REPLY,
        ),
        IpAddr::V6(_) => (
            Domain::IPV6,
            Protocol::ICMPV6,
            ICMPV6_ECHO_REQUEST,
            ICMPV6_ECHO_REPLY,
        ),
    };
    let icmp_socket = Socket::new(domain, Type::DGRAM, Some(protocol))
        .and_then(|icmp_socket| {
            icmp_socket.set_nonblocking(true)?;
            UdpSocket::from_std(StdUdpSocket::from(icmp_socket))
        })
        .map_err(|denied| LatencyProbeError::IcmpSocketDenied(denied.to_string()))?;
    let exchange = async {
        icmp_socket.connect(SocketAddr::new(host, 0)).await?;
        let sent_at = Instant::now();
        icmp_socket
            .send(&echo_request(request_kind, sequence))
            .await?;
        let mut reply_buffer = [0_u8; 256];
        loop {
            let received_length = icmp_socket.recv(&mut reply_buffer).await?;
            if is_echo_reply(&reply_buffer[..received_length], reply_kind, sequence) {
                return Ok::<Duration, io::Error>(sent_at.elapsed());
            }
        }
    };
    Ok(match timeout(PROBE_TIMEOUT, exchange).await {
        Ok(Ok(round_trip)) => ProbeOutcome::Answered(round_trip),
        Ok(Err(_)) | Err(_) => ProbeOutcome::Lost,
    })
}

fn echo_request(request_kind: u8, sequence: u16) -> Vec<u8> {
    let [sequence_high, sequence_low] = sequence.to_be_bytes();
    [request_kind, 0, 0, 0, 0, 0, sequence_high, sequence_low]
        .into_iter()
        .chain(ECHO_PAYLOAD.iter().copied())
        .collect()
}

fn is_echo_reply(reply_bytes: &[u8], reply_kind: u8, sequence: u16) -> bool {
    reply_bytes.len() >= 8
        && reply_bytes[0] == reply_kind
        && u16::from_be_bytes([reply_bytes[6], reply_bytes[7]]) == sequence
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_of(outcomes: &[ProbeOutcome]) -> ProbeWindow {
        outcomes.iter().fold(
            ProbeWindow::empty(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            |window, outcome| window.recorded(*outcome),
        )
    }

    #[test]
    fn loss_share_counts_lost_probes_in_the_window() {
        let window = window_of(&[
            ProbeOutcome::Answered(Duration::from_millis(10)),
            ProbeOutcome::Lost,
            ProbeOutcome::Answered(Duration::from_millis(30)),
            ProbeOutcome::Lost,
        ]);
        assert!((window.loss_share().get::<ratio>() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn median_ignores_lost_probes() {
        let window = window_of(&[
            ProbeOutcome::Answered(Duration::from_millis(40)),
            ProbeOutcome::Lost,
            ProbeOutcome::Answered(Duration::from_millis(10)),
            ProbeOutcome::Answered(Duration::from_millis(20)),
        ]);
        assert_eq!(window.median_round_trip(), Some(Duration::from_millis(20)));
    }

    #[test]
    fn window_keeps_only_the_latest_probes() {
        let outcomes = vec![ProbeOutcome::Lost; WINDOW_PROBES + 5];
        assert_eq!(window_of(&outcomes).outcomes.len(), WINDOW_PROBES);
    }

    #[tokio::test]
    #[ignore = "needs net.ipv4.ping_group_range to include the test user"]
    async fn loopback_answers_an_unprivileged_echo() {
        let outcome = echo_round_trip(IpAddr::V4(Ipv4Addr::LOCALHOST), 7)
            .await
            .unwrap();
        assert!(matches!(outcome, ProbeOutcome::Answered(_)));
    }

    #[test]
    fn echo_reply_matches_kind_and_sequence() {
        let mut reply_bytes = echo_request(ICMPV4_ECHO_REPLY, 513);
        assert!(is_echo_reply(&reply_bytes, ICMPV4_ECHO_REPLY, 513));
        assert!(!is_echo_reply(&reply_bytes, ICMPV4_ECHO_REPLY, 514));
        reply_bytes[0] = ICMPV4_ECHO_REQUEST;
        assert!(!is_echo_reply(&reply_bytes, ICMPV4_ECHO_REPLY, 513));
    }
}
