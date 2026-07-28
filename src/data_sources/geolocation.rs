//! Location from the situo daemon's session-bus interface `dev.situo.Location1`
//! (its own reverse-geocoder: Wi-Fi → `BeaconDB` → place). Read over D-Bus with
//! zbus's blocking API, so it fits a sync poll. `None` when the daemon is
//! absent or has no valid fix — which takes the whole weather group off the
//! bar.

use zbus::blocking::{Connection, Proxy};

#[derive(Debug, Clone)]
pub struct Geo {
    pub latitude: f64,
    pub longitude: f64,
    pub place_name: String,
    pub place_country: String,
    pub place_distance_km: f64,
    pub accuracy_m: f64,
    pub source: String,
    pub description: String,
}

impl Geo {
    /// The current fix, or `None` if situo is unavailable or its fix is
    /// invalid.
    pub fn resolve() -> Option<Self> {
        let connection = Connection::session().ok()?;
        let proxy = Proxy::new(
            &connection,
            "dev.situo",
            "/dev/situo/Location1",
            "dev.situo.Location1",
        )
        .ok()?;
        let valid: bool = proxy.get_property("Valid").ok()?;
        if !valid {
            return None;
        }
        Some(Geo {
            latitude: proxy.get_property("Latitude").ok()?,
            longitude: proxy.get_property("Longitude").ok()?,
            place_name: proxy.get_property("PlaceName").unwrap_or_default(),
            place_country: proxy.get_property("PlaceCountry").unwrap_or_default(),
            place_distance_km: proxy.get_property("PlaceDistanceKm").unwrap_or(f64::NAN),
            accuracy_m: proxy.get_property("Accuracy").unwrap_or(f64::NAN),
            source: proxy.get_property("Source").unwrap_or_default(),
            description: proxy.get_property("Description").unwrap_or_default(),
        })
    }

    /// `--lat <n> --lon <n>` query params for the weather APIs.
    pub fn query(&self) -> String {
        format!("latitude={}&longitude={}", self.latitude, self.longitude)
    }
}
