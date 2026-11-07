use sottomano_desktop_sources::screen_brightness_color::clightd_backlight::{
    self, Backlight, Smooth, poll_all,
};

use crate::screen_brightness_color::clight_calibration_writes;

#[derive(Debug, thiserror::Error)]
pub enum BacklightWriteError {
    #[error("clightd is not reachable on the system bus")]
    DaemonUnreachable,
    #[error("clightd call failed: {0}")]
    Call(#[from] zbus::Error),
    #[error("clightd refused to set the backlight")]
    Refused,
}

pub fn set(serial: &str, percent: i64) -> Result<(), BacklightWriteError> {
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
        if let Err(halt_failure) = clightd_set(&serial, frac) {
            tracing::warn!("backlight {serial}: halting the ramp: {halt_failure}");
        }
    }
}

fn clightd_set(serial: &str, frac: f64) -> Result<(), BacklightWriteError> {
    let proxy = clightd_backlight::blocking_backlight_proxy()
        .ok_or(BacklightWriteError::DaemonUnreachable)?;
    let accepted = proxy.set(
        frac,
        Smooth {
            enabled: false,
            step: 0.0,
            timeout: 0,
        },
        serial,
    )?;
    if accepted {
        Ok(())
    } else {
        Err(BacklightWriteError::Refused)
    }
}
