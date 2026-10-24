use futures::{Stream, StreamExt};
use notulist_client::{DICTATION_BUS_NAME, DictationEngine, followed_dictation_engine};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

#[derive(Debug, Clone, PartialEq)]
pub struct NotulistEngine {
    pub engine: DictationEngine,
}

impl ReadingSource for NotulistEngine {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "notulist engine";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let session = buses.session;
        name_owned(session.clone(), DICTATION_BUS_NAME).while_held(
            move || pushed_delivery_stream(dictation_engines(session.clone())),
            SourceDelivery::Absent,
        )
    }
}

async fn dictation_engines(
    session: zbus::Connection,
) -> Result<impl Stream<Item = Result<NotulistEngine, Error>> + Send, Error> {
    Ok(followed_dictation_engine(&session)
        .await?
        .map(|engine| Ok(NotulistEngine { engine: engine? })))
}
