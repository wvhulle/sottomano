use sottomano_desktop_sources::screen_brightness_color::gammarelay_color_temperature::{
    self, TEMP_RANGE,
};

use crate::sink_write_errors::BusWriteError;

const GAMMA_RELAY_SERVICE: &str = "wl-gammarelay";

pub async fn set_brightness(percent: i64) -> Result<(), BusWriteError> {
    let frac = percent.clamp(0, 100) as f64 / 100.0;
    let proxy = gammarelay_color_temperature::session_gamma_relay()
        .await
        .map_err(|_| BusWriteError::ServiceUnreachable(GAMMA_RELAY_SERVICE))?;
    Ok(proxy.set_brightness(frac).await?)
}

pub async fn set_temperature(kelvin: u16) -> Result<(), BusWriteError> {
    let clamped = kelvin.clamp(*TEMP_RANGE.start(), *TEMP_RANGE.end());
    let proxy = gammarelay_color_temperature::session_gamma_relay()
        .await
        .map_err(|_| BusWriteError::ServiceUnreachable(GAMMA_RELAY_SERVICE))?;
    Ok(proxy.set_temperature(clamped).await?)
}
