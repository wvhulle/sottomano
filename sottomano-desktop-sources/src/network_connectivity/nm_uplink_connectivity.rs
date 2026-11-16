use futures::{Stream, StreamExt};

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::SnapshotDeliveryStreams,
    network_connectivity::{
        network_manager_proxies::{self, Connectivity},
        nm_samples,
    },
    system_protocol_clients::dbus_desktop_buses::{BusReadError, DesktopBuses},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UplinkConnectivity {
    pub connectivity: Connectivity,
}

impl SnapshotSource for UplinkConnectivity {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Connectivity";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        nm_samples(
            buses.system,
            network_manager_proxies::CONNECTIVITY_PROPERTIES,
            network_manager_proxies::manager_connectivity,
        )
        .map(|sample| {
            SnapshotDelivery::live_push(sample.map(|connectivity| Self { connectivity }))
                .unavailable_when_absent()
        })
        .repeats_as_unchanged()
    }
}
