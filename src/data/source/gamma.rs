//! Software gamma via `wl-gammarelay` (user bus): a global dim (the
//! `Brightness` property, 0..1, a lossy color-ramp scale — not the hardware
//! backlight) and color temperature (`Temperature`, Kelvin). Both live on the
//! daemon's **root** object (`/`), where a write applies to every output at
//! once — exactly the aggregate one-dim / one-warmth control the group wants.
//! `None` when wl-gammarelay is absent (no external displays under gamma
//! control).

use std::{ops::RangeInclusive, time::Duration};

use zbus::{
    blocking::{Connection, Proxy},
    zvariant::Value as DbusValue,
};

use crate::{
    data::source::{BarWidget, Source, TooltipBody, Unit, Value, row},
    widgets::icon::Icon,
};

const DEST: &str = "rs.wl-gammarelay";
const PATH: &str = "/";
const IFACE: &str = "rs.wl.gammarelay";

/// Slider range for color temperature (warm → cool), in Kelvin.
pub const TEMP_RANGE: RangeInclusive<u16> = 3000..=6500;

#[derive(Debug, Clone)]
pub struct Gamma {
    /// Software dim, as a percentage (100 = no dimming).
    pub brightness: i64,
    /// Color temperature, in Kelvin.
    pub temperature: u16,
}

impl BarWidget for Gamma {
    fn icon(&self) -> Icon {
        Icon::CircleHalfStroke
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
        let proxy = Proxy::new(&connection, DEST, PATH, IFACE).ok()?;
        let brightness: f64 = proxy.get_property("Brightness").ok()?;
        let temperature: u16 = proxy.get_property("Temperature").ok()?;
        #[allow(clippy::cast_possible_truncation)]
        Some(Gamma {
            brightness: (brightness * 100.0).round() as i64,
            temperature,
        })
    }
}

/// Set the software dim across all outputs.
pub fn set_brightness(percent: i64) -> bool {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    set_property("Brightness", frac)
}

/// Set the color temperature (Kelvin) across all outputs.
pub fn set_temperature(kelvin: u16) -> bool {
    let clamped = kelvin.clamp(*TEMP_RANGE.start(), *TEMP_RANGE.end());
    set_property("Temperature", clamped)
}

/// Write a property on wl-gammarelay's global root object.
fn set_property<T>(name: &str, value: T) -> bool
where
    T: Into<DbusValue<'static>> + 'static,
{
    let Ok(connection) = Connection::session() else {
        return false;
    };
    let Ok(proxy) = Proxy::new(&connection, DEST, PATH, IFACE) else {
        return false;
    };
    proxy.set_property(name, value).is_ok()
}
