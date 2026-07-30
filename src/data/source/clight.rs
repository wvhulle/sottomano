//! clight's webcam auto-calibration state, over the **user** bus
//! (`org.clight.clight`). The indicator shows whether auto-brightness is on
//! (eye) or the backlight is under manual control (eye-slash); toggling flips
//! clight's `NoAutoCalib` and persists the choice to `~/.config/clight.conf` so
//! it survives a clight restart (every rebuild restarts the user service).
//!
//! Switching to auto fires an immediate webcam `Capture` instead of waiting for
//! clight's next scheduled one; switching to manual halts any in-flight clightd
//! ramp (see [`backlight::halt_ramp`]) so the level freezes where it is. `None`
//! when clight is absent, which simply hides the toggle.
//!
//! [`backlight::halt_ramp`]: crate::data::source::backlight::halt_ramp

use std::{env, fs, time::Duration};

use zbus::blocking::{Connection, Proxy};

use crate::{
    data::source::{BarWidget, Source, TooltipBody, backlight, row},
    widgets::{icon::Icon, toggle_control::Toggle},
};

const DEST: &str = "org.clight.clight";
const CONF_BACKLIGHT_PATH: &str = "/org/clight/clight/Conf/Backlight";
const CONF_BACKLIGHT_IFACE: &str = "org.clight.clight.Conf.Backlight";
const CLIGHT_PATH: &str = "/org/clight/clight";
const CLIGHT_IFACE: &str = "org.clight.clight";

#[derive(Debug, Clone)]
pub struct Clight {
    /// Whether webcam auto-calibration is driving the backlight.
    pub auto: bool,
}

impl BarWidget for Clight {
    fn icon(&self) -> Icon {
        if self.auto { Icon::Eye } else { Icon::EyeSlash }
    }

    /// Dim when under manual control — the auto pipeline is idle.
    fn disabled(&self) -> bool {
        !self.auto
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::Table(vec![row(
            "Auto-brightness",
            if self.auto { "on" } else { "off — manual" },
        )])
    }
}

impl Source for Clight {
    const NAME: &'static str = "clight";
    const INTERVAL: Duration = Duration::from_secs(10);

    fn poll() -> Option<Self> {
        no_auto_calib().map(|no| Clight { auto: !no })
    }
}

impl Toggle for Clight {
    fn toggled(&self) -> Self {
        Clight { auto: !self.auto }
    }

    fn toggle() -> bool {
        flip()
    }
}

/// Read clight's `NoAutoCalib` property; `None` when clight is unavailable.
fn no_auto_calib() -> Option<bool> {
    let connection = Connection::session().ok()?;
    let proxy = Proxy::new(&connection, DEST, CONF_BACKLIGHT_PATH, CONF_BACKLIGHT_IFACE).ok()?;
    proxy.get_property("NoAutoCalib").ok()
}

fn set_no_auto_calib(value: bool) -> bool {
    let Ok(connection) = Connection::session() else {
        return false;
    };
    let Ok(proxy) = Proxy::new(&connection, DEST, CONF_BACKLIGHT_PATH, CONF_BACKLIGHT_IFACE) else {
        return false;
    };
    proxy.set_property("NoAutoCalib", value).is_ok()
}

/// Fire an immediate webcam capture so a switch-to-auto adjusts at once rather
/// than on clight's next scheduled tick.
fn capture() {
    let Ok(connection) = Connection::session() else {
        return;
    };
    if let Ok(proxy) = Proxy::new(&connection, DEST, CLIGHT_PATH, CLIGHT_IFACE) {
        let _: zbus::Result<()> = proxy.call("Capture", &(true, false));
    }
}

/// Persist the auto/manual choice to `~/.config/clight.conf`. clight reads this
/// natively on startup, so the value survives a service restart. The file holds
/// only this one key; everything else is merged from the system config.
fn persist(manual: bool) {
    let dir = env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|dir| !dir.is_empty())
        .or_else(|| env::var("HOME").ok().map(|home| format!("{home}/.config")));
    let Some(dir) = dir else {
        return;
    };
    let value = if manual { "true" } else { "false" };
    let contents = format!("backlight:\n{{\n    no_auto_calibration = {value};\n}};\n");
    let _ = fs::write(format!("{dir}/clight.conf"), contents);
}

/// Pause auto-calibration (used before a manual brightness set so the webcam
/// doesn't fight the change), persisting the choice.
pub fn pause_auto_calib() {
    set_no_auto_calib(true);
    persist(true);
}

/// Flip auto ↔ manual: set `NoAutoCalib`, then capture (→ auto) or halt the
/// clightd ramp (→ manual), and persist. Returns whether the property write
/// succeeded.
fn flip() -> bool {
    let currently_auto = no_auto_calib().is_some_and(|no| !no);
    // Toggling away from auto means switching to manual.
    let manual = currently_auto;
    let ok = set_no_auto_calib(manual);
    if manual {
        backlight::halt_ramp();
    } else {
        capture();
    }
    persist(manual);
    ok
}
