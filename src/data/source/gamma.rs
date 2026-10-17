use std::{ops::RangeInclusive, time::Duration};

use iced::futures::{Stream, StreamExt};
use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature};
use zbus::{blocking::Connection, proxy};

use crate::data::source::{
    DeliveryCadence, ReadingSource, SourceDelivery, bus,
    bus::{Error, name_owned},
    polled_delivery_stream, present_or_absent,
};

const RELAY_SERVICE: &str = "rs.wl-gammarelay";

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

impl ReadingSource for Gamma {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "gamma";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(5));

    fn delivery_stream(key: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        present_or_absent(name_owned(bus::Bus::Session, RELAY_SERVICE), move || {
            polled_delivery_stream(key, Self::poll).boxed()
        })
    }
}

impl Gamma {
    async fn poll((): ()) -> Result<Self, Error> {
        let proxy: GammaRelayProxy = bus::session_proxy().await?;
        let brightness = proxy.brightness().await?;
        let temperature = proxy.temperature().await?;
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
