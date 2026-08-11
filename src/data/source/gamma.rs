use std::{ops::RangeInclusive, time::Duration};

use tokio::task::spawn_blocking;
use zbus::{blocking::Connection, proxy};

use crate::data::source::{
    BarWidget, Glyph, Source, TooltipBody, Unit, Value, bus, bus::Error, font_awesome::Solid, row,
};

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
    pub temperature: u16,
}

impl BarWidget for Gamma {
    fn icon(&self) -> Glyph {
        Solid::CircleHalfStroke.into()
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.brightness,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![
            row("Dim", format!("{}%", self.brightness)),
            row("Temperature", format!("{} K", self.temperature)),
        ])
    }
}

impl Source for Gamma {
    type Error = Error;

    const NAME: &'static str = "gamma";
    const INTERVAL: Duration = Duration::from_secs(5);

    async fn poll() -> Result<Option<Self>, Self::Error> {
        spawn_blocking(read_gamma)
            .await
            .map_err(|_| Error::Panicked)?
    }
}

fn read_gamma() -> Result<Option<Gamma>, Error> {
    let connection = Connection::session().map_err(|error| Error::Connect(error.to_string()))?;
    let proxy = GammaRelayProxyBlocking::new(&connection)
        .map_err(|error| Error::Connect(error.to_string()))?;
    let Some(brightness) = bus::call(proxy.brightness())? else {
        return Ok(None);
    };
    let Some(temperature) = bus::call(proxy.temperature())? else {
        return Ok(None);
    };
    #[allow(clippy::cast_possible_truncation)]
    Ok(Some(Gamma {
        brightness: (brightness * 100.0).round() as i64,
        temperature,
    }))
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
