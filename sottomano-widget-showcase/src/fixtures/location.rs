use jiff::Timestamp;
use situo_client::{
    AccessPointExclusions, BeaconDbEstimate, CellRadio, DecidingSource, FixOrigin, GeoCoordinates,
    LocationFix, NearestPlace, ObservationCounts, PositionEstimate, RadioSource, ServingCell,
    SourceLookup, SourceLookupOutcome, UpdateSchedule, UpdateStatus, UpdateTrigger,
};
use sottomano_desktop_sources::{SnapshotDelivery, weather::situo_location_fix::Location};
use sottomano_widgets::{
    clock::Clock,
    indicator_groups::location_group::{LocationGroup, LocationMessage},
};
use uom::si::{
    angle::degree,
    f64::{Angle, Length},
    length::{kilometer, meter},
};

use crate::showcased_group::ShowcasedGroup;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut location_group = LocationGroup::default();
    let _ = location_group.update(
        LocationMessage::Location(SnapshotDelivery::from(porto_location(clock))),
        clock,
    );
    ShowcasedGroup::Location(Box::new(location_group))
}

pub(crate) fn porto_location(clock: Clock) -> Location {
    Location {
        geo: porto_location_fix(clock),
        update_schedule: UpdateSchedule::from_wire(
            900,
            30,
            [
                UpdateTrigger::ServingCellChange,
                UpdateTrigger::WifiNeighbourhoodChange,
                UpdateTrigger::NetworkReconnect,
                UpdateTrigger::ModemHotplug,
            ],
        ),
        update_status: UpdateStatus::Idle,
    }
}

fn porto_location_fix(clock: Clock) -> LocationFix {
    LocationFix {
        position: GeoCoordinates {
            latitude: Angle::new::<degree>(41.1496),
            longitude: Angle::new::<degree>(-8.6110),
        },
        origin: FixOrigin::Estimated(BeaconDbEstimate {
            decided_by: DecidingSource::Radio(RadioSource::Wifi),
            accuracy: Length::new::<meter>(305.0),
            source_lookups: vec![
                SourceLookup {
                    source: RadioSource::Wifi,
                    outcome: SourceLookupOutcome::Located(PositionEstimate {
                        position: GeoCoordinates {
                            latitude: Angle::new::<degree>(41.1496),
                            longitude: Angle::new::<degree>(-8.6110),
                        },
                        accuracy: Length::new::<meter>(305.0),
                    }),
                },
                SourceLookup {
                    source: RadioSource::Cell,
                    outcome: SourceLookupOutcome::Located(PositionEstimate {
                        position: GeoCoordinates {
                            latitude: Angle::new::<degree>(41.1532),
                            longitude: Angle::new::<degree>(-8.6051),
                        },
                        accuracy: Length::new::<meter>(1_250.0),
                    }),
                },
            ],
            serving_cells: vec![ServingCell {
                radio: CellRadio::Lte,
                mobile_country_code: 268,
                mobile_network_code: 1,
                location_area_code: 0x1F4A,
                cell_id: 0x0310_2C05,
            }],
            observations: ObservationCounts {
                access_points_seen: 17,
                access_points_submitted: 15,
                access_point_exclusions: AccessPointExclusions {
                    opted_out: 0,
                    hidden: 1,
                    mobile_hotspot: 1,
                    below_lookup_minimum: 0,
                },
                cells_seen: 1,
                cells_submitted: 1,
            },
        }),
        place: Some(NearestPlace {
            name: "Porto".to_owned(),
            country: "PT".to_owned(),
            distance: Length::new::<kilometer>(0.48),
        }),
        timestamp: Timestamp::from_second(clock.utc().timestamp())
            .expect("the showcase clock is within jiff's range"),
    }
}
