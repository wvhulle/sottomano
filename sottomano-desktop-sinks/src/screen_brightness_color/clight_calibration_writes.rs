use std::{fs, path::Path};

use sottomano_desktop_sources::{
    screen_brightness_color::clight_auto_brightness::{
        BrightnessCalibration, ClightDaemonProxy, ConfBacklightProxy, config_file,
        current_calibration,
    },
    system_protocol_clients::dbus_desktop_buses::BusReadError,
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
    if let Err(write_failure) = fs::write::<&Path, String>(&file, contents) {
        tracing::warn!(
            "clight: persisting the calibration to {}: {write_failure}",
            file.display()
        );
    }
}

pub async fn pause_auto_calib() {
    if let Err(pause_failure) = set_calibration(BrightnessCalibration::Fixed).await {
        tracing::debug!("clight: pausing webcam calibration: {pause_failure}");
    }
    persist(BrightnessCalibration::Fixed);
}

pub async fn flip() -> Result<(), BusReadError> {
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

async fn set_calibration(calibration: BrightnessCalibration) -> Result<(), BusReadError> {
    let session = Connection::session().await?;
    ConfBacklightProxy::new(&session)
        .await?
        .set_no_auto_calib(calibration == BrightnessCalibration::Fixed)
        .await?;
    Ok(())
}

async fn capture() {
    let captured = async {
        ClightDaemonProxy::new(&Connection::session().await?)
            .await?
            .capture(true, false)
            .await
    };
    if let Err(capture_failure) = captured.await {
        tracing::warn!("clight: capturing webcam light: {capture_failure}");
    }
}
