pub mod huggingface_model_card;
pub mod notulist_dbus_dictation;
pub mod notulist_dbus_engine;
pub mod notulist_dbus_output_problems;
pub mod notulist_dbus_pipeline_activity;

use futures::{Stream, StreamExt};
use notulist_client::DICTATION_BUS_NAME;

use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::StreamOperators,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

fn notulist_followed_deliveries<S, Value, Followed, FollowedValues>(
    buses: DesktopBuses,
    follow: impl Fn(zbus::Connection) -> Followed + Send + 'static,
    snapshot_of: fn(Value) -> S,
) -> impl Stream<Item = SnapshotDelivery<S>> + Send
where
    S: SnapshotSource<Error = Error> + PartialEq,
    Followed: Future<Output = zbus::Result<FollowedValues>> + Send,
    FollowedValues: Stream<Item = zbus::Result<Value>> + Send,
{
    let session = buses.session;
    name_owned(session.clone(), DICTATION_BUS_NAME).available_while(move || {
        let followed = follow(session.clone());
        SnapshotDelivery::live_push_deliveries(async move {
            Ok(followed.await?.map(move |value| Ok(snapshot_of(value?))))
        })
    })
}
