use futures::Stream;
use notulist_client::{OutputProblem, followed_output_problems};

use crate::{
    SnapshotDelivery, SnapshotSource,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error},
    voice_dictation_input::notulist_followed_deliveries,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotulistOutputProblems {
    pub problems: Vec<OutputProblem>,
}

impl SnapshotSource for NotulistOutputProblems {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Notulist output problems";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        notulist_followed_deliveries(
            buses,
            |session| async move { followed_output_problems(&session).await },
            |problems| NotulistOutputProblems { problems },
        )
    }
}
