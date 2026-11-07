use futures::{Stream, StreamExt};

use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, InterfaceOffer},
    voice_dictation_input::notulist_interface_offers,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotulistInterfaceMismatch {
    pub expected_interface: String,
    pub offered_versions: Vec<String>,
}

impl SnapshotSource for NotulistInterfaceMismatch {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Notulist interface version";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        notulist_interface_offers(buses.session)
            .map(|offer| match offer {
                Some(Ok(InterfaceOffer::OtherVersionsOffered {
                    expected_interface,
                    offered_versions,
                })) => SnapshotDelivery::live_push(Ok(NotulistInterfaceMismatch {
                    expected_interface,
                    offered_versions,
                })),
                Some(Ok(InterfaceOffer::ExpectedInterfaceOffered) | Err(_)) | None => {
                    SnapshotDelivery::SourceUnavailable
                }
            })
            .distinct_until_changed()
    }
}
