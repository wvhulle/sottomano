use futures::{Stream, StreamExt};
use notulist_client::{DICTATION_BUS_NAME, OutputProblem, followed_output_problems};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotulistOutputProblems {
    pub problems: Vec<OutputProblem>,
}

impl ReadingSource for NotulistOutputProblems {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "notulist output problems";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let session = buses.session;
        name_owned(session.clone(), DICTATION_BUS_NAME).while_held(
            move || pushed_delivery_stream(output_problem_sets(session.clone())),
            SourceDelivery::Absent,
        )
    }
}

async fn output_problem_sets(
    session: zbus::Connection,
) -> Result<impl Stream<Item = Result<NotulistOutputProblems, Error>> + Send, Error> {
    Ok(followed_output_problems(&session).await?.map(|problems| {
        Ok(NotulistOutputProblems {
            problems: problems?,
        })
    }))
}
