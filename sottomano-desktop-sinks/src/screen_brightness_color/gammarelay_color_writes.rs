use sottomano_desktop_sources::screen_brightness_color::gammarelay_color_temperature::{
    self, TEMP_RANGE,
};

#[must_use]
pub fn set_brightness(percent: i64) -> bool {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    gammarelay_color_temperature::gamma_relay_proxy()
        .is_some_and(|proxy| proxy.set_brightness(frac).is_ok())
}

#[must_use]
pub fn set_temperature(kelvin: u16) -> bool {
    let clamped = kelvin.clamp(*TEMP_RANGE.start(), *TEMP_RANGE.end());
    gammarelay_color_temperature::gamma_relay_proxy()
        .is_some_and(|proxy| proxy.set_temperature(clamped).is_ok())
}
