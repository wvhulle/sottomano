use std::time::Duration;

use futures::{Stream, StreamExt, TryStreamExt, stream};
use situo_client::{
    FixOrigin, GeoCoordinates, LocationFix, ReadLocationFix, UpdateSchedule, location_fix_updates,
};
use uom::si::time::second;
use zbus::Connection;

use super::Error;
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub geo: LocationFix,
    pub update_schedule: UpdateSchedule,
}

impl Location {
    #[must_use]
    pub const fn position(&self) -> GeoCoordinates {
        self.geo.position
    }

    #[must_use]
    pub fn fallback_interval(&self) -> Duration {
        Duration::from_secs_f64(self.update_schedule.fallback_interval.get::<second>())
    }

    #[must_use]
    pub fn cell_check_interval(&self) -> Duration {
        Duration::from_secs_f64(self.update_schedule.cell_check_interval.get::<second>())
    }

    #[must_use]
    pub fn delivery_cadence(&self) -> DeliveryCadence {
        match self.geo.origin {
            FixOrigin::Manual | FixOrigin::Pinned => DeliveryCadence::LivePush,
            FixOrigin::Estimated(_) => DeliveryCadence::LivePushWithRecheck(
                PeriodicPolling::every(self.fallback_interval()),
            ),
        }
    }
}

impl From<Location> for SnapshotDelivery<Location> {
    fn from(location: Location) -> Self {
        let delivery_cadence = location.delivery_cadence();
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch: Ok(location),
            delivery_cadence,
        }
    }
}

async fn read_update_schedule() -> Result<UpdateSchedule, Error> {
    let system_bus = Connection::system()
        .await
        .map_err(situo_client::ClientError::from)?;
    Ok(system_bus.read_update_schedule().await?)
}

impl SnapshotSource for Location {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Location";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(read_update_schedule())
            .map_ok(|update_schedule| {
                location_fix_updates()
                    .map_err(Error::from)
                    .map_ok(move |geo| Location {
                        geo,
                        update_schedule: update_schedule.clone(),
                    })
            })
            .try_flatten()
            .map(|location_fetch| match location_fetch {
                Ok(location) => SnapshotDelivery::from(location),
                Err(fetch_error) => SnapshotDelivery::live_push(Err(fetch_error)),
            })
            .map(|delivery| {
                delivery.unavailable_when(|fetch_error| {
                    matches!(
                        fetch_error,
                        Error::Situo(situo_client::ClientError::NoDaemon)
                    )
                })
            })
    }
}
