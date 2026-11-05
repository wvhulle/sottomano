use futures::{Stream, StreamExt};
use notulist_client::{DICTATION_BUS_NAME, PipelineActivity, followed_pipeline_activity};

use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

#[derive(Debug, Clone, PartialEq)]
pub struct NotulistPipeline {
    pub activity: PipelineActivity,
}

impl SnapshotSource for NotulistPipeline {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Notulist pipeline";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let session = buses.session;
        name_owned(session.clone(), DICTATION_BUS_NAME).while_held(
            move || SnapshotDelivery::live_push_deliveries(pipeline_activities(session.clone())),
            SnapshotDelivery::SourceUnavailable,
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
