use std::ops::RangeInclusive;

use futures::{Stream, StreamExt, stream};
use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature};
use zbus::{blocking::Connection, proxy};

use crate::{
    SnapshotDelivery, SnapshotSource,
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
pub trait GammaRelay {
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
pub struct ColorTemperature {
    pub brightness: i64,
    pub temperature: ThermodynamicTemperature,
}

impl SnapshotSource for ColorTemperature {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Color temperature";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        name_owned(buses.session.clone(), RELAY_SERVICE).while_held(
            move || SnapshotDelivery::live_push_deliveries(relay_changes(buses.clone())),
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

async fn relay_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<ColorTemperature, Error>> + Send, Error> {
    let relay: GammaRelayProxy = buses.proxy(dbus_desktop_buses::Bus::Session).await?;
    let changes = stream::select(
        relay.receive_brightness_changed().await.map(drop),
        relay.receive_temperature_changed().await.map(drop),
    );
    Ok(changes.then(move |()| {
        let relay = relay.clone();
        async move { ColorTemperature::read(&relay).await }
    }))
}

impl ColorTemperature {
    async fn read(relay: &GammaRelayProxy<'_>) -> Result<Self, Error> {
        let brightness = relay.brightness().await?;
        let temperature = relay.temperature().await?;
        #[allow(clippy::cast_possible_truncation)]
        Ok(ColorTemperature {
            brightness: (brightness * 100.0).round() as i64,
            temperature: ThermodynamicTemperature::new::<thermodynamic_temperature::kelvin>(
                f64::from(temperature),
            ),
        })
    }
}

#[must_use]
pub fn gamma_relay_proxy() -> Option<GammaRelayProxyBlocking<'static>> {
    GammaRelayProxyBlocking::new(&Connection::session().ok()?).ok()
}
