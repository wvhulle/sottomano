pub mod huggingface_model_card;
pub mod notulist_dbus_dictation;
pub mod notulist_dbus_engine;
pub mod notulist_dbus_interface_mismatch;
pub mod notulist_dbus_output_problems;
pub mod notulist_dbus_pipeline_activity;

use futures::{Stream, StreamExt, stream};
use notulist_client::{DICTATION_BUS_NAME, DICTATION_INTERFACE, DICTATION_OBJECT_PATH};

use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::{Availability, StreamOperators},
    system_protocol_clients::dbus_desktop_buses::{
        DesktopBuses, Error, InterfaceOffer, introspected_interface_offer, name_owned,
    },
};

fn notulist_interface_offers(
    session: zbus::Connection,
) -> impl Stream<Item = Option<Result<InterfaceOffer, Error>>> + Send {
    name_owned(session.clone(), DICTATION_BUS_NAME).switch_map(move |owner_availability| {
        let session = session.clone();
        stream::once(async move {
            match owner_availability {
                Availability::Available => Some(
                    introspected_interface_offer(
                        &session,
                        DICTATION_BUS_NAME,
                        DICTATION_OBJECT_PATH,
                        DICTATION_INTERFACE,
                    )
                    .await,
                ),
                Availability::Unavailable => None,
            }
        })
    })
}

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
    notulist_interface_offers(session.clone())
        .map(|offer| match offer {
            Some(Ok(InterfaceOffer::ExpectedInterfaceOffered) | Err(_)) => Availability::Available,
            Some(Ok(InterfaceOffer::OtherVersionsOffered { .. })) | None => {
                Availability::Unavailable
            }
        })
        .available_while(move || {
            let followed = follow(session.clone());
            SnapshotDelivery::live_push_deliveries(async move {
                Ok(followed.await?.map(move |value| Ok(snapshot_of(value?))))
            })
        })
}
