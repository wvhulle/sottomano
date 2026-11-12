use futures::{Stream, StreamExt};
use futures_rx::RxExt;

use crate::{
    SnapshotDelivery, SnapshotSource, SourceCaption,
    network_connectivity::{
        network_manager_proxies::{self, Connectivity},
        nm_readings,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UplinkConnectivity {
    pub connectivity: Connectivity,
}

impl SnapshotSource for UplinkConnectivity {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Connectivity";
    const CAPTION: SourceCaption = SourceCaption::Captioned(
        "NetworkManager's own check whether the internet answers or a login portal is in the way.",
    );

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        nm_readings(
            buses.system,
            network_manager_proxies::CONNECTIVITY_PROPERTIES,
            network_manager_proxies::manager_connectivity,
        )
        .map(|reading| match reading {
            Ok(Some(connectivity)) => SnapshotDelivery::live_push(Ok(Self { connectivity })),
            Ok(None) => SnapshotDelivery::SourceUnavailable,
            Err(failure) => SnapshotDelivery::live_push(Err(failure)),
        })
        .distinct_until_changed()
    }
}
