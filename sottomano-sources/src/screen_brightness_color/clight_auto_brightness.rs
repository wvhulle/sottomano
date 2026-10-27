use std::{
    env, fs,
    path::{Path, PathBuf},
};

use futures::Stream;
use zbus::{blocking::Connection, proxy, proxy::CacheProperties};

use crate::{
    SnapshotDelivery, SnapshotSource,
    screen_brightness_color::clightd_backlight,
    stream_operators::{StreamOperators, read_once_then_per_burst},
    system_protocol_clients::{
        dbus_desktop_buses,
        dbus_desktop_buses::{DesktopBuses, Error, name_owned},
        inotify_directory_changes::{WatchTarget, changes},
    },
};

const CLIGHT_SERVICE: &str = "org.clight.clight";

#[proxy(
    interface = "org.clight.clight.Conf.Backlight",
    default_service = "org.clight.clight",
    default_path = "/org/clight/clight/Conf/Backlight"
)]
trait ConfBacklight {
    #[zbus(property)]
    fn no_auto_calib(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_no_auto_calib(&self, value: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.clight.clight",
    default_service = "org.clight.clight",
    default_path = "/org/clight/clight"
)]
trait ClightDaemon {
    fn capture(&self, reset_timer: bool, capture_only: bool) -> zbus::Result<()>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutoBrightness {
    pub auto: bool,
}

impl SnapshotSource for AutoBrightness {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "clight";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        name_owned(buses.session.clone(), CLIGHT_SERVICE).while_held(
            move || SnapshotDelivery::live_push_deliveries(calibration_changes(buses.clone())),
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

async fn calibration_changes(
    buses: DesktopBuses,
) -> Result<impl Stream<Item = Result<AutoBrightness, Error>> + Send, Error> {
    let conf = ConfBacklightProxy::builder(buses.on(dbus_desktop_buses::Bus::Session))
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let config_written = changes(config_file().map(WatchTarget::File).into_iter().collect());
    Ok(read_once_then_per_burst(config_written, move || {
        let conf = conf.clone();
        async move {
            Ok(AutoBrightness {
                auto: !conf.no_auto_calib().await?,
            })
        }
    }))
}

fn config_file() -> Option<PathBuf> {
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::home_dir().map(|home| home.join(".config")))?;
    Some(config_home.join("clight.conf"))
}

fn no_auto_calib() -> Option<bool> {
    let connection = Connection::session().ok()?;
    ConfBacklightProxyBlocking::new(&connection)
        .ok()?
        .no_auto_calib()
        .ok()
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
        clightd_backlight::halt_ramp();
    } else {
        capture();
    }
    persist(manual);
    outcome
}
