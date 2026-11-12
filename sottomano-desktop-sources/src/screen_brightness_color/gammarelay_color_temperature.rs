use std::ops::RangeInclusive;

use futures::Stream;
use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature};
use zbus::{Connection, proxy};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::ChangeTriggerStreams,
    network_connectivity::interface_properties_changed,
    system_protocol_clients::dbus_desktop_buses::{DesktopBuses, Error, name_owned},
};

const RELAY_SERVICE: &str = "rs.wl-gammarelay";
const RELAY_INTERFACE: &str = "rs.wl.gammarelay";

#[proxy(
    interface = "rs.wl.gammarelay",
    default_service = "rs.wl-gammarelay",
    default_path = "/",
    gen_blocking = false
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
        name_owned(buses.session.clone(), RELAY_SERVICE).while_available(
            move || {
                let buses = buses.clone();
                SnapshotDelivery::live_push_deliveries(buses, relay_changes)
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

async fn relay_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<ColorTemperature, Error>> + Send, Error> {
    let relay = GammaRelayProxy::new(&buses.session).await?;
    let changes =
        interface_properties_changed(&buses.session, RELAY_SERVICE, RELAY_INTERFACE).await?;
    Ok(changes.triggered_rereads(move || {
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

pub async fn session_gamma_relay() -> zbus::Result<GammaRelayProxy<'static>> {
    GammaRelayProxy::new(&Connection::session().await?).await
}
