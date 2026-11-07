use sottomano_desktop_sources::screen_brightness_color::gammarelay_color_temperature::{
    self, TEMP_RANGE,
};

#[derive(Debug, thiserror::Error)]
pub enum GammaRelayWriteError {
    #[error("wl-gammarelay is not reachable on the session bus")]
    RelayUnreachable,
    #[error("wl-gammarelay call failed: {0}")]
    Call(#[from] zbus::Error),
}

pub fn set_brightness(percent: i64) -> Result<(), GammaRelayWriteError> {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    let proxy = gammarelay_color_temperature::gamma_relay_proxy()
        .ok_or(GammaRelayWriteError::RelayUnreachable)?;
    Ok(proxy.set_brightness(frac)?)
}

pub fn set_temperature(kelvin: u16) -> Result<(), GammaRelayWriteError> {
    let clamped = kelvin.clamp(*TEMP_RANGE.start(), *TEMP_RANGE.end());
    let proxy = gammarelay_color_temperature::gamma_relay_proxy()
        .ok_or(GammaRelayWriteError::RelayUnreachable)?;
    Ok(proxy.set_temperature(clamped)?)
}
