use std::{ops::RangeInclusive, time::Duration};

use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature};
use zbus::{blocking::Connection, proxy};

use crate::data::source::{Source, bus, bus::Error};

#[proxy(
    interface = "rs.wl.gammarelay",
    default_service = "rs.wl-gammarelay",
    default_path = "/"
)]
trait GammaRelay {
    #[zbus(property)]
    fn brightness(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn set_brightness(&self, value: f64) -> zbus::Result<()>;
    #[zbus(property)]
    fn temperature(&self) -> zbus::Result<u16>;
    #[zbus(property)]
    fn set_temperature(&self, value: u16) -> zbus::Result<()>;
}

pub const TEMP_RANGE: RangeInclusive<u16> = 3000..=6500;

#[derive(Debug, Clone, PartialEq)]
pub struct Gamma {
    pub brightness: i64,
    pub temperature: ThermodynamicTemperature,
}

impl Source for Gamma {
    type Error = Error;

    type Key = ();

    const NAME: &'static str = "gamma";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn available((): &()) -> bool {
        Self::poll(()).await.is_ok()
    }

    async fn poll((): ()) -> Result<Self, Self::Error> {
        let connection = zbus::Connection::session()
            .await
            .map_err(|error| Error::Connect(error.to_string()))?;
        let proxy = GammaRelayProxy::new(&connection)
            .await
            .map_err(|error| Error::Connect(error.to_string()))?;
        let brightness = bus::call(proxy.brightness().await)?;
        let temperature = bus::call(proxy.temperature().await)?;
        #[allow(clippy::cast_possible_truncation)]
        Ok(Gamma {
            brightness: (brightness * 100.0).round() as i64,
            temperature: ThermodynamicTemperature::new::<thermodynamic_temperature::kelvin>(
                f64::from(temperature),
            ),
        })
    }
}

pub fn set_brightness(percent: i64) -> bool {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    relay().is_some_and(|proxy| proxy.set_brightness(frac).is_ok())
}

pub fn set_temperature(kelvin: u16) -> bool {
    let clamped = kelvin.clamp(*TEMP_RANGE.start(), *TEMP_RANGE.end());
    relay().is_some_and(|proxy| proxy.set_temperature(clamped).is_ok())
}

fn relay() -> Option<GammaRelayProxyBlocking<'static>> {
    GammaRelayProxyBlocking::new(&Connection::session().ok()?).ok()
}
