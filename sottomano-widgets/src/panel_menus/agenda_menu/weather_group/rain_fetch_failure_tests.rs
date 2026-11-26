use std::time::Duration;

use sottomano_desktop_sources::{
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    weather::{
        self,
        buienradar_rain::{Downpour, precipitation::millimeter_per_hour},
    },
};
use uom::si::f64::Velocity;

use super::*;

const RAIN_CADENCE: DeliveryCadence =
    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(60)));

fn delivered_rain(clock: Clock) -> SnapshotDelivery<Rain> {
    SnapshotDelivery::fixture_delivered(
        Ok(Rain::try_from(vec![Downpour {
            intensity: Velocity::new::<millimeter_per_hour>(3.0),
            at: clock.local(),
        }])
        .unwrap()),
        RAIN_CADENCE,
    )
}

fn failed_rain(error: weather::Error) -> SnapshotDelivery<Rain> {
    SnapshotDelivery::fixture_delivered(Err(error), RAIN_CADENCE)
}

#[test]
fn a_failed_rain_fetch_keeps_the_last_forecast() {
    let clock = Clock::real();
    let mut group = WeatherGroup::default();
    let _ = group.update(WeatherMessage::Rain(delivered_rain(clock)), clock);
    let _ = group.update(
        WeatherMessage::Rain(failed_rain(weather::Error::Malformed)),
        clock,
    );
    assert!(group.rain.get().is_some());
}

#[test]
fn a_rain_failure_without_an_earlier_forecast_shows_as_failed() {
    let clock = Clock::real();
    let mut group = WeatherGroup::default();
    let _ = group.update(
        WeatherMessage::Rain(failed_rain(weather::Error::OutsideRainRadar)),
        clock,
    );
    assert!(group.rain.failure().is_some());
}
