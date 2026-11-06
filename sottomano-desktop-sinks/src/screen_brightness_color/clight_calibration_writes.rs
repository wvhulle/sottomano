use std::{fs, path::Path};

use sottomano_desktop_sources::{
    screen_brightness_color::clight_auto_brightness::{
        ClightDaemonProxyBlocking, ConfBacklightProxyBlocking, config_file, no_auto_calib,
    },
    system_protocol_clients::dbus_desktop_buses::Error,
};
use zbus::blocking::Connection;

use crate::screen_brightness_color::clightd_backlight_writes;

fn persist(manual: bool) {
    let Some(file) = config_file() else {
        return;
    };
    let value = if manual { "true" } else { "false" };
    let contents = format!("backlight:\n{{\n    no_auto_calibration = {value};\n}};\n");
    let _ = fs::write::<&Path, String>(&file, contents);
}

pub fn pause_auto_calib() {
    let _ = set_no_auto_calib(true);
    persist(true);
}

pub fn flip() -> Result<(), Error> {
    let currently_auto = no_auto_calib().is_some_and(|no| !no);
    let manual = currently_auto;
    let outcome = set_no_auto_calib(manual);
    if manual {
        clightd_backlight_writes::halt_ramp();
    } else {
        capture();
    }
    persist(manual);
    outcome
}

fn set_no_auto_calib(value: bool) -> Result<(), Error> {
    let connection = Connection::session()?;
    ConfBacklightProxyBlocking::new(&connection)?.set_no_auto_calib(value)?;
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
