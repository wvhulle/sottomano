use std::{ops::RangeInclusive, time::Duration};

use zbus::{blocking::Connection, proxy};

use crate::data::source::{
    BarWidget, Glyph, Source, TooltipBody, Unit, Value, font_awesome::Solid, row,
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
    const NAME: &'static str = "gamma";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn poll() -> Option<Self> {
        let connection = Connection::session().ok()?;
        let proxy = GammaRelayProxyBlocking::new(&connection).ok()?;
        let brightness = proxy.brightness().ok()?;
        let temperature = proxy.temperature().ok()?;
        #[allow(clippy::cast_possible_truncation)]
        Some(Gamma {
            brightness: (brightness * 100.0).round() as i64,
            temperature,
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
