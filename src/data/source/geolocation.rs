//! Location from the situo daemon's session-bus interface `dev.situo.Location1`
//! (its own reverse-geocoder: Wi-Fi → `BeaconDB` → place). Read over D-Bus with
//! zbus's blocking API, so it fits a sync poll. `None` when the daemon is
//! absent or has no valid fix — which takes the whole weather group off the
//! bar.

use zbus::{blocking::Connection, proxy};

#[proxy(
    interface = "dev.situo.Location1",
    default_service = "dev.situo",
    default_path = "/dev/situo/Location1"
)]
trait Location1 {
    #[zbus(property)]
    fn valid(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn latitude(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn longitude(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn place_name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn place_country(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn place_distance_km(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn accuracy(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn source(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn description(&self) -> zbus::Result<String>;
}

#[derive(Debug, Clone, PartialEq)]
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
        let proxy = Location1ProxyBlocking::new(&connection).ok()?;
        if !proxy.valid().ok()? {
            return None;
        }
        Some(Geo {
            latitude: proxy.latitude().ok()?,
            longitude: proxy.longitude().ok()?,
            place_name: proxy.place_name().unwrap_or_default(),
            place_country: proxy.place_country().unwrap_or_default(),
            place_distance_km: proxy.place_distance_km().unwrap_or(f64::NAN),
            accuracy_m: proxy.accuracy().unwrap_or(f64::NAN),
            source: proxy.source().unwrap_or_default(),
            description: proxy.description().unwrap_or_default(),
        })
    }

    /// `--lat <n> --lon <n>` query params for the weather APIs.
    pub fn query(&self) -> String {
        format!("latitude={}&longitude={}", self.latitude, self.longitude)
    }
}
