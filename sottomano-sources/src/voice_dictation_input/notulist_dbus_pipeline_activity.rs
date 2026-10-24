use futures::{Stream, StreamExt};
use notulist_client::{DICTATION_BUS_NAME, PipelineActivity, followed_pipeline_activity};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

#[derive(Debug, Clone, PartialEq)]
pub struct NotulistPipeline {
    pub activity: PipelineActivity,
}

impl ReadingSource for NotulistPipeline {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "notulist pipeline";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let session = buses.session;
        name_owned(session.clone(), DICTATION_BUS_NAME).while_held(
            move || pushed_delivery_stream(pipeline_activities(session.clone())),
            SourceDelivery::Absent,
        )
    }
}

async fn pipeline_activities(
    session: zbus::Connection,
) -> Result<impl Stream<Item = Result<NotulistPipeline, Error>> + Send, Error> {
    Ok(followed_pipeline_activity(&session).await?.map(|activity| {
        Ok(NotulistPipeline {
            activity: activity?,
        })
    }))
}
