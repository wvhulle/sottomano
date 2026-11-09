use futures::{StreamExt, stream};
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

pub async fn set(serial: String, percent: i64) -> Result<(), BacklightWriteError> {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    clight_calibration_writes::pause_auto_calib().await;
    clightd_set(&serial, frac).await
}

pub async fn halt_ramp() {
    stream::iter(poll_all().await)
        .for_each(
            async |Backlight {
                       serial, percent, ..
                   }| {
                let current_fraction = f64::from(i32::try_from(percent).unwrap_or(0)) / 100.0;
                if let Err(halt_failure) = clightd_set(&serial, current_fraction).await {
                    tracing::warn!("backlight {serial}: halting the ramp: {halt_failure}");
                }
            },
        )
        .await;
}

async fn clightd_set(serial: &str, frac: f64) -> Result<(), BacklightWriteError> {
    let proxy = clightd_backlight::system_backlight_proxy()
        .await
        .map_err(|_| BacklightWriteError::DaemonUnreachable)?;
    let instant_change = Smooth {
        enabled: false,
        step: 0.0,
        timeout: 0,
    };
    if proxy.set(frac, instant_change, serial).await? {
        Ok(())
    } else {
        Err(BacklightWriteError::Refused)
    }
}
