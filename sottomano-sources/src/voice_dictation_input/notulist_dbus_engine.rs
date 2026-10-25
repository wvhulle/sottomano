use futures::{Stream, StreamExt};
use notulist_client::{DICTATION_BUS_NAME, EngineState, followed_engine_state};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

#[derive(Debug, Clone, PartialEq)]
pub struct NotulistEngine {
    pub engine: EngineState,
}

impl ReadingSource for NotulistEngine {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "notulist engine";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let session = buses.session;
        name_owned(session.clone(), DICTATION_BUS_NAME).while_held(
            move || pushed_delivery_stream(engine_states(session.clone())),
            SourceDelivery::Absent,
        )
    }
}

async fn engine_states(
    session: zbus::Connection,
) -> Result<impl Stream<Item = Result<NotulistEngine, Error>> + Send, Error> {
    Ok(followed_engine_state(&session)
        .await?
        .map(|engine| Ok(NotulistEngine { engine: engine? })))
}
