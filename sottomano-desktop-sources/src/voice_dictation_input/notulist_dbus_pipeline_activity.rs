use futures::Stream;
use notulist_client::{PipelineActivity, followed_pipeline_activity};

use crate::{
    SnapshotDelivery, SnapshotSource,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error},
    voice_dictation_input::notulist_followed_deliveries,
};

#[derive(Debug, Clone, PartialEq)]
pub struct NotulistPipeline {
    pub activity: PipelineActivity,
}

impl SnapshotSource for NotulistPipeline {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Pipeline";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        notulist_followed_deliveries(
            buses,
            |session| async move { followed_pipeline_activity(&session).await },
            |activity| NotulistPipeline { activity },
        )
    }
}
