use sottomano_desktop_sources::screen_brightness_color::clightd_backlight::{
    self, Backlight, Smooth, poll_all,
};

use crate::screen_brightness_color::clight_calibration_writes;

#[must_use]
pub fn set(serial: &str, percent: i64) -> bool {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    clight_calibration_writes::pause_auto_calib();
    clightd_set(serial, frac)
}

pub fn halt_ramp() {
    for Backlight {
        serial, percent, ..
    } in poll_all()
    {
        let frac = f64::from(i32::try_from(percent).unwrap_or(0)) / 100.0;
        let _ = clightd_set(&serial, frac);
    }
}

fn clightd_set(serial: &str, frac: f64) -> bool {
    let Some(proxy) = clightd_backlight::blocking_backlight_proxy() else {
        return false;
    };
    proxy
        .set(
            frac,
            Smooth {
                enabled: false,
                step: 0.0,
                timeout: 0,
            },
            serial,
        )
        .unwrap_or(false)
}
