use std::{collections::BTreeMap, mem, pin::Pin};

use futures::{
    Stream, StreamExt,
    future::join_all,
    stream::{self, select},
};

use crate::{
    SnapshotDelivery, SnapshotSource, SourceCaption,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling, SnapshotDeliveryStreams},
    network_connectivity::{
        icmp_echo_probes::{
            LatencyProbeError, PROBE_POLLING, ProbeOutcome, ProbeWindow, echo_round_trip,
            probe_address,
        },
        network_manager_proxies::{self, DeviceGateway},
        nm_readings,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayLatency {
    pub interface: String,
    pub window: ProbeWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayLatencies {
    pub gateways: Vec<GatewayLatency>,
}

impl GatewayLatencies {
    pub fn of_interface<'latencies>(
        &'latencies self,
        interface: &'latencies str,
    ) -> impl Iterator<Item = &'latencies GatewayLatency> {
        self.gateways
            .iter()
            .filter(move |gateway_latency| gateway_latency.interface == interface)
    }
}

impl SnapshotSource for GatewayLatencies {
    type Error = LatencyProbeError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Router latency";
    const CAPTION: SourceCaption = SourceCaption::Captioned(
        "Echo round trips every five seconds to each router this link sends traffic through. Slow \
         or lost replies here point at the local network rather than the provider.",
    );

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let gateway_changes = nm_readings(
            buses.system,
            network_manager_proxies::GATEWAY_PROPERTIES,
            network_manager_proxies::device_gateways,
        )
        .map(|gateways| ProbeEvent::GatewaysChanged(gateways.ok().flatten().unwrap_or_default()));
        let ticks = PROBE_POLLING.poll_ticks().map(ProbeEvent::Tick);
        let probe_events: Pin<Box<dyn Stream<Item = ProbeEvent> + Send>> =
            Box::pin(select(gateway_changes, ticks));
        stream::unfold(
            (probe_events, GatewayProber::default()),
            |(mut probe_events, mut prober)| async move {
                loop {
                    match probe_events.next().await? {
                        ProbeEvent::GatewaysChanged(gateways) => prober.retarget(gateways),
                        ProbeEvent::Tick(polling) => {
                            let delivery = prober.probe_round(polling).await;
                            return Some((delivery, (probe_events, prober)));
                        }
                    }
                }
            },
        )
        .repeats_as_unchanged()
    }
}

enum ProbeEvent {
    GatewaysChanged(Vec<DeviceGateway>),
    Tick(PeriodicPolling),
}

#[derive(Default)]
struct GatewayProber {
    windows: BTreeMap<DeviceGateway, ProbeWindow>,
    next_sequence: u16,
}

impl GatewayProber {
    fn retarget(&mut self, gateways: Vec<DeviceGateway>) {
        let mut previous_windows = mem::take(&mut self.windows);
        self.windows = gateways
            .into_iter()
            .map(|device_gateway| {
                let window = previous_windows
                    .remove(&device_gateway)
                    .unwrap_or_else(|| ProbeWindow::empty(device_gateway.gateway));
                (device_gateway, window)
            })
            .collect();
    }

    async fn probe_round(
        &mut self,
        polling: PeriodicPolling,
    ) -> SnapshotDelivery<GatewayLatencies> {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let outcomes: Vec<Result<ProbeOutcome, LatencyProbeError>> =
            join_all(self.windows.keys().map(|device_gateway| {
                echo_round_trip(
                    probe_address(device_gateway.gateway, &device_gateway.interface),
                    sequence,
                )
            }))
            .await;
        let outcomes = match outcomes.into_iter().collect::<Result<Vec<_>, _>>() {
            Ok(outcomes) => outcomes,
            Err(failure) => {
                return SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Err(failure),
                    delivery_cadence: DeliveryCadence::PeriodicPolling(polling),
                };
            }
        };
        for (window, outcome) in self.windows.values_mut().zip(outcomes) {
            *window = window.clone().recorded(outcome);
        }
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch: Ok(GatewayLatencies {
                gateways: self
                    .windows
                    .iter()
                    .map(|(device_gateway, window)| GatewayLatency {
                        interface: device_gateway.interface.clone(),
                        window: window.clone(),
                    })
                    .collect(),
            }),
            delivery_cadence: DeliveryCadence::PeriodicPolling(polling),
        }
    }
}
