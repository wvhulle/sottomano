use std::{fs, path::Path};

use sottomano_desktop_sources::{
    screen_brightness_color::clight_auto_brightness::{
        BrightnessCalibration, ClightDaemonProxy, ConfBacklightProxy, config_file,
        current_calibration,
    },
    system_protocol_clients::dbus_desktop_buses::Error,
};
use zbus::Connection;

use crate::screen_brightness_color::clightd_backlight_writes;

fn persist(calibration: BrightnessCalibration) {
    let Some(file) = config_file() else {
        return;
    };
    let value = match calibration {
        BrightnessCalibration::Fixed => "true",
        BrightnessCalibration::FromWebcam => "false",
    };
    let contents = format!("backlight:\n{{\n    no_auto_calibration = {value};\n}};\n");
    let _ = fs::write::<&Path, String>(&file, contents);
}

pub async fn pause_auto_calib() {
    let _ = set_calibration(BrightnessCalibration::Fixed).await;
    persist(BrightnessCalibration::Fixed);
}

pub async fn flip() -> Result<(), Error> {
    let wanted_calibration = match current_calibration().await {
        Ok(BrightnessCalibration::FromWebcam) => BrightnessCalibration::Fixed,
        Ok(BrightnessCalibration::Fixed) => BrightnessCalibration::FromWebcam,
        Err(read_failure) => {
            tracing::warn!(
                "clight: calibration unknown, enabling webcam calibration: {read_failure}"
            );
            BrightnessCalibration::FromWebcam
        }
    };
    let outcome = set_calibration(wanted_calibration).await;
    match wanted_calibration {
        BrightnessCalibration::Fixed => clightd_backlight_writes::halt_ramp().await,
        BrightnessCalibration::FromWebcam => capture().await,
    }
    persist(wanted_calibration);
    outcome
}

async fn set_calibration(calibration: BrightnessCalibration) -> Result<(), Error> {
    let session = Connection::session().await?;
    ConfBacklightProxy::new(&session)
        .await?
        .set_no_auto_calib(calibration == BrightnessCalibration::Fixed)
        .await?;
    Ok(())
}

async fn capture() {
    let Ok(session) = Connection::session().await else {
        return;
    };
    if let Ok(proxy) = ClightDaemonProxy::new(&session).await {
        let _ = proxy.capture(true, false).await;
    }
}
