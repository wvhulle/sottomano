use std::future::Future;

use futures::{Stream, StreamExt, stream};

use super::{DeliveryCadence, SnapshotDeliveryStreams};
use crate::{
    SnapshotDelivery, SnapshotSource, system_protocol_clients::protocol_peer_reconnects::subscribed,
};

impl<Source: SnapshotSource> SnapshotDelivery<Source> {
    pub fn live_push(snapshot_fetch: Result<Source, Source::Error>) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            delivery_cadence: DeliveryCadence::LivePush,
        }
    }

    pub fn single_read(snapshot_fetch: Result<Source, Source::Error>) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            delivery_cadence: DeliveryCadence::SingleRead,
        }
    }

    pub fn live_push_deliveries<ConnectInputs, ReadStream, ConnectFuture>(
        connect_inputs: ConnectInputs,
        connect: impl FnMut(ConnectInputs) -> ConnectFuture + Send,
    ) -> impl Stream<Item = Self> + Send
    where
        ConnectInputs: Clone + Send,
        Source: PartialEq,
        ReadStream: Stream<Item = Result<Source, Source::Error>> + Send,
        ConnectFuture: Future<Output = Result<ReadStream, Source::Error>> + Send,
    {
        subscribed(connect_inputs, connect)
            .map(SnapshotDelivery::live_push)
            .repeats_as_unchanged()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn fixture_delivered(
        snapshot_fetch: Result<Source, Source::Error>,
        delivery_cadence: DeliveryCadence,
    ) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            delivery_cadence,
        }
    }

    pub fn single_read_deliveries(
        read: impl Future<Output = Result<Source, Source::Error>> + Send,
    ) -> impl Stream<Item = Self> + Send {
        stream::once(read).map(SnapshotDelivery::single_read)
    }
}
