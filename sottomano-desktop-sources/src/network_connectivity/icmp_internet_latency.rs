use std::{
    future::ready,
    net::{IpAddr, SocketAddr},
};

use futures::{Stream, StreamExt};
use futures_rx::RxExt;

use crate::{
    SnapshotDelivery, SnapshotSource, SourceCaption,
    delivery_cadence_drivers::DeliveryCadence,
    network_connectivity::icmp_echo_probes::{
        LatencyProbeError, PROBE_POLLING, ProbeWindow, echo_round_trip,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternetLatency {
    pub window: ProbeWindow,
}

impl SnapshotSource for InternetLatency {
    type Error = LatencyProbeError;
    type StreamInputs = IpAddr;

    const NAME: &'static str = "Internet latency";
    const CAPTION: SourceCaption = SourceCaption::Captioned(
        "Echo round trips every five seconds to a host on the internet, through whichever \
         connection carries the traffic.",
    );

    fn delivery_stream(internet_host: IpAddr) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PROBE_POLLING
            .poll_ticks()
            .enumerate()
            .then(move |(tick_count, polling)| async move {
                #[allow(clippy::cast_possible_truncation)]
                let sequence = tick_count as u16;
                let outcome = echo_round_trip(SocketAddr::new(internet_host, 0), sequence).await;
                (polling, outcome)
            })
            .scan(
                ProbeWindow::empty(internet_host),
                |window, (polling, outcome)| {
                    let snapshot_fetch = match outcome {
                        Ok(outcome) => {
                            *window = window.clone().recorded(outcome);
                            Ok(InternetLatency {
                                window: window.clone(),
                            })
                        }
                        Err(failure) => Err(failure),
                    };
                    ready(Some(SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch,
                        delivery_cadence: DeliveryCadence::PeriodicPolling(polling),
                    }))
                },
            )
            .distinct_until_changed()
    }
}
