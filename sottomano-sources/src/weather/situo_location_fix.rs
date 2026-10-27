use futures::{Stream, StreamExt};
use situo_client::{GeoCoordinates, LocationFix, location_fix_updates};

use super::Error;
use crate::{SnapshotDelivery, SnapshotSource};

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub geo: LocationFix,
}

impl Location {
    #[must_use]
    pub const fn position(&self) -> GeoCoordinates {
        self.geo.position
    }
}

impl SnapshotSource for Location {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "location";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        location_fix_updates()
            .map(|update| update.map(|geo| Self { geo }).map_err(Error::from))
            .map(SnapshotDelivery::live_push)
    }
}
