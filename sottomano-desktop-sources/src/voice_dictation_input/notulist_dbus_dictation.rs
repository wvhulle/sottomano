use futures::Stream;
use notulist_client::{DictationPhase, followed_dictation_phases};

use crate::{
    SnapshotDelivery, SnapshotSource,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error},
    voice_dictation_input::notulist_followed_deliveries,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotulistDictation {
    pub phase: DictationPhase,
}

impl SnapshotSource for NotulistDictation {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Notulist dictation";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        notulist_followed_deliveries(
            buses,
            |session| async move { followed_dictation_phases(&session).await },
            |phase| NotulistDictation { phase },
        )
    }
}
