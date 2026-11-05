use std::{
    future::ready,
    net::{IpAddr, SocketAddr},
};

use futures::{Stream, StreamExt};

use crate::{
    SnapshotDelivery, SnapshotSource, SourceCaption,
    network_connectivity::icmp_echo_probes::{
        LatencyProbeError, PROBE_POLLING, ProbeWindow, echo_round_trip,
    },
    stream_operators::StreamOperators,
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
            .then(move |(tick_count, poll_tick)| async move {
                #[allow(clippy::cast_possible_truncation)]
                let sequence = tick_count as u16;
                let outcome = echo_round_trip(SocketAddr::new(internet_host, 0), sequence).await;
                (poll_tick, outcome)
            })
            .scan(
                ProbeWindow::empty(internet_host),
                |window, (poll_tick, outcome)| {
                    let delivery = match outcome {
                        Ok(outcome) => {
                            *window = window.clone().recorded(outcome);
                            poll_tick.stamped(Ok(InternetLatency {
                                window: window.clone(),
                            }))
                        }
                        Err(failure) => poll_tick.stamped(Err(failure)),
                    };
                    ready(Some(delivery))
                },
            )
            .distinct_until_changed()
    }
}
