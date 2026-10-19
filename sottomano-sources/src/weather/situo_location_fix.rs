use futures::{Stream, StreamExt};
use situo_client::{Fix, Position, fix_updates};

use super::Error;
use crate::{DeliveryCadence, ReadingSource, SourceDelivery};

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub geo: Fix,
}

impl Location {
    #[must_use]
    pub const fn position(&self) -> Position {
        self.geo.position
    }
}

impl ReadingSource for Location {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "location";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(_key: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        fix_updates()
            .map(|update| update.map(|geo| Self { geo }).map_err(Error::from))
            .map(SourceDelivery::Reading)
    }
}
