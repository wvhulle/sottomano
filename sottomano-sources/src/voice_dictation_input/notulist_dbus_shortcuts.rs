use futures::{Stream, StreamExt};
use notulist_client::{DICTATION_BUS_NAME, DictationShortcutBinding, followed_shortcut_bindings};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotulistShortcuts {
    pub bindings: Vec<DictationShortcutBinding>,
}

impl ReadingSource for NotulistShortcuts {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "notulist shortcuts";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let session = buses.session;
        name_owned(session.clone(), DICTATION_BUS_NAME).while_held(
            move || pushed_delivery_stream(shortcut_bindings(session.clone())),
            SourceDelivery::Absent,
        )
    }
}

async fn shortcut_bindings(
    session: zbus::Connection,
) -> Result<impl Stream<Item = Result<NotulistShortcuts, Error>> + Send, Error> {
    Ok(followed_shortcut_bindings(&session).await?.map(|bindings| {
        Ok(NotulistShortcuts {
            bindings: bindings?,
        })
    }))
}
