use futures::Stream;
use notulist_client::{EngineState, followed_engine_state};

use crate::{
    SnapshotDelivery, SnapshotSource,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error},
    voice_dictation_input::notulist_followed_deliveries,
};

#[derive(Debug, Clone, PartialEq)]
pub struct NotulistEngine {
    pub engine: EngineState,
}

impl SnapshotSource for NotulistEngine {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Notulist engine";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        notulist_followed_deliveries(
            buses,
            |session| async move { followed_engine_state(&session).await },
            |engine| NotulistEngine { engine },
        )
    }
}
