use futures::{Stream, StreamExt, future::ready, stream};
use notulist_client::{
    DICTATION_BUS_NAME, DictationCommand, DictationPhase, DictationShortcutBinding,
    followed_dictation_phases, followed_shortcut_bindings, send_dictation_command,
};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotulistDictation {
    pub phase: DictationPhase,
    pub shortcuts: Vec<DictationShortcutBinding>,
}

enum DictationUpdate {
    Phase(DictationPhase),
    Shortcuts(Vec<DictationShortcutBinding>),
}

#[derive(Default)]
struct DictationFold {
    phase: Option<DictationPhase>,
    shortcuts: Vec<DictationShortcutBinding>,
}

impl DictationFold {
    fn apply(&mut self, update: DictationUpdate) -> Option<NotulistDictation> {
        match update {
            DictationUpdate::Phase(phase) => self.phase = Some(phase),
            DictationUpdate::Shortcuts(shortcuts) => self.shortcuts = shortcuts,
        }
        Some(NotulistDictation {
            phase: self.phase?,
            shortcuts: self.shortcuts.clone(),
        })
    }
}

impl ReadingSource for NotulistDictation {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "notulist";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let session = buses.session;
        name_owned(session.clone(), DICTATION_BUS_NAME).while_held(
            move || pushed_delivery_stream(dictation_readings(session.clone())),
            SourceDelivery::Absent,
        )
    }
}

async fn dictation_readings(
    session: zbus::Connection,
) -> Result<impl Stream<Item = Result<NotulistDictation, Error>> + Send, Error> {
    let phases = followed_dictation_phases(&session)
        .await?
        .map(|phase| phase.map(DictationUpdate::Phase));
    let shortcuts = followed_shortcut_bindings(&session)
        .await?
        .map(|shortcuts| shortcuts.map(DictationUpdate::Shortcuts));
    let mut fold = DictationFold::default();
    Ok(stream::select(phases, shortcuts).filter_map(move |update| {
        ready(match update {
            Ok(update) => fold.apply(update).map(Ok),
            Err(error) => Some(Err(error.into())),
        })
    }))
}

pub async fn send_logged_dictation_command(command: DictationCommand) {
    if let Err(error) = send_dictation_command(command).await {
        tracing::warn!("notulist {command:?}: {error}");
    }
}
