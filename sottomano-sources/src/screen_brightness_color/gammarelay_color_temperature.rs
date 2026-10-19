use std::ops::RangeInclusive;

use futures::{Stream, StreamExt, stream};
use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature};
use zbus::{blocking::Connection, proxy};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, pushed_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::{
        dbus_desktop_buses,
        dbus_desktop_buses::{DesktopBuses, Error, name_owned},
    },
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
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "gamma";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        name_owned(buses.session.clone(), RELAY_SERVICE).while_held(
            move || pushed_delivery_stream(relay_changes(buses.clone())),
            SourceDelivery::Absent,
        )
    }
}

async fn relay_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<Gamma, Error>> + Send, Error> {
    let relay: GammaRelayProxy = buses.proxy(dbus_desktop_buses::Bus::Session).await?;
    let changes = stream::select(
        relay.receive_brightness_changed().await.map(drop),
        relay.receive_temperature_changed().await.map(drop),
    );
    Ok(changes.then(move |()| {
        let relay = relay.clone();
        async move { Gamma::read(&relay).await }
    }))
}

impl Gamma {
    async fn read(relay: &GammaRelayProxy<'_>) -> Result<Self, Error> {
        let brightness = relay.brightness().await?;
        let temperature = relay.temperature().await?;
        #[allow(clippy::cast_possible_truncation)]
        Ok(Gamma {
            brightness: (brightness * 100.0).round() as i64,
            temperature: ThermodynamicTemperature::new::<thermodynamic_temperature::kelvin>(
                f64::from(temperature),
            ),
        })
    }
}

#[must_use]
pub fn set_brightness(percent: i64) -> bool {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    relay().is_some_and(|proxy| proxy.set_brightness(frac).is_ok())
}

#[must_use]
pub fn set_temperature(kelvin: u16) -> bool {
    let clamped = kelvin.clamp(*TEMP_RANGE.start(), *TEMP_RANGE.end());
    relay().is_some_and(|proxy| proxy.set_temperature(clamped).is_ok())
}

fn relay() -> Option<GammaRelayProxyBlocking<'static>> {
    GammaRelayProxyBlocking::new(&Connection::session().ok()?).ok()
}
