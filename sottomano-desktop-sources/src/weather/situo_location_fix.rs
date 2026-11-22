use std::time::Duration;

use futures::{Stream, StreamExt};
use situo_client::{
    FixOrigin, GeoCoordinates, LocationFix, LocationUpdate, LocationUpdates, UpdateSchedule,
    UpdateStatus,
};
use uom::si::time::second;

use super::Error;
use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub geo: LocationFix,
    pub update_schedule: UpdateSchedule,
    pub update_status: UpdateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForecastGrid {
    OpenMeteo,
    Buienradar,
}

impl ForecastGrid {
    const fn cell_size_degrees(self) -> f64 {
        match self {
            Self::OpenMeteo => 0.01,
            Self::Buienradar => 0.001,
        }
    }

    fn snapped(self, position: GeoCoordinates) -> GeoCoordinates {
        let cell_size = self.cell_size_degrees();
        let snap_degrees = |degrees: f64| (degrees / cell_size).round() * cell_size;
        GeoCoordinates::from_degrees(
            snap_degrees(position.latitude_degrees()),
            snap_degrees(position.longitude_degrees()),
        )
        .unwrap_or(position)
    }
}

impl Location {
    #[must_use]
    pub const fn position(&self) -> GeoCoordinates {
        self.geo.position
    }

    #[must_use]
    pub fn position_on(&self, forecast_grid: ForecastGrid) -> GeoCoordinates {
        forecast_grid.snapped(self.geo.position)
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

impl From<LocationUpdate> for Location {
    fn from(location_update: LocationUpdate) -> Self {
        Self {
            geo: location_update.fix,
            update_schedule: location_update.update_schedule,
            update_status: location_update.update_status,
        }
    }
}

impl SnapshotSource for Location {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Location";

    fn delivery_stream(_key: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        LocationUpdates::default()
            .map(|location_fetch| match location_fetch {
                Ok(location_update) => SnapshotDelivery::from(Location::from(location_update)),
                Err(fetch_error) => SnapshotDelivery::live_push(Err(Error::from(fetch_error))),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buienradar_keeps_a_300_metre_move_that_open_meteo_merges() {
        let station = GeoCoordinates::from_degrees(51.0361, 3.7104).unwrap();
        let three_hundred_metres_north = GeoCoordinates::from_degrees(51.0388, 3.7104).unwrap();
        assert_eq!(
            ForecastGrid::OpenMeteo.snapped(station),
            ForecastGrid::OpenMeteo.snapped(three_hundred_metres_north)
        );
        assert_ne!(
            ForecastGrid::Buienradar.snapped(station),
            ForecastGrid::Buienradar.snapped(three_hundred_metres_north)
        );
    }
}
