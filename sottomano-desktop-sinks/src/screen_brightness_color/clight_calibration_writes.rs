use std::{fs, path::Path};

use sottomano_desktop_sources::{
    screen_brightness_color::clight_auto_brightness::{
        BrightnessCalibration, ClightDaemonProxyBlocking, ConfBacklightProxyBlocking, config_file,
        current_calibration,
    },
    system_protocol_clients::dbus_desktop_buses::Error,
};
use zbus::blocking::Connection;

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

pub fn pause_auto_calib() {
    let _ = set_calibration(BrightnessCalibration::Fixed);
    persist(BrightnessCalibration::Fixed);
}

pub fn flip() -> Result<(), Error> {
    let wanted_calibration = match current_calibration() {
        Some(BrightnessCalibration::FromWebcam) => BrightnessCalibration::Fixed,
        Some(BrightnessCalibration::Fixed) | None => BrightnessCalibration::FromWebcam,
    };
    let outcome = set_calibration(wanted_calibration);
    match wanted_calibration {
        BrightnessCalibration::Fixed => clightd_backlight_writes::halt_ramp(),
        BrightnessCalibration::FromWebcam => capture(),
    }
    persist(wanted_calibration);
    outcome
}

fn set_calibration(calibration: BrightnessCalibration) -> Result<(), Error> {
    let connection = Connection::session()?;
    ConfBacklightProxyBlocking::new(&connection)?
        .set_no_auto_calib(calibration == BrightnessCalibration::Fixed)?;
    Ok(())
}

fn capture() {
    let Ok(connection) = Connection::session() else {
        return;
    };
    if let Ok(proxy) = ClightDaemonProxyBlocking::new(&connection) {
        let _ = proxy.capture(true, false);
    }
}
