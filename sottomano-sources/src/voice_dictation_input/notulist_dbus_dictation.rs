use futures::{Stream, StreamExt};
use notulist_client::{
    DICTATION_BUS_NAME, DictationCommand, DictationPhase, followed_dictation_phases,
    send_dictation_command,
};

use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
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
        let session = buses.session;
        name_owned(session.clone(), DICTATION_BUS_NAME).while_held(
            move || SnapshotDelivery::live_push_deliveries(dictation_phases(session.clone())),
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

async fn dictation_phases(
    session: zbus::Connection,
) -> Result<impl Stream<Item = Result<NotulistDictation, Error>> + Send, Error> {
    Ok(followed_dictation_phases(&session)
        .await?
        .map(|phase| Ok(NotulistDictation { phase: phase? })))
}

pub async fn send_logged_dictation_command(command: DictationCommand) {
    if let Err(error) = send_dictation_command(command).await {
        tracing::warn!("notulist {command:?}: {error}");
    }
}
