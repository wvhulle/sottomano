use std::{env, fs, time::Duration};

use iced::futures::{Stream, StreamExt};
use zbus::{blocking::Connection, proxy};

use crate::data::source::{
    Delivery, PolledSource, Source, backlight, bus,
    bus::{Error, name_owned},
    present_or_absent, ticked_polls,
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
pub struct Clight {
    pub auto: bool,
}

impl Source for Clight {
    type Error = Error;
    type Key = ();

    const NAME: &'static str = "clight";
}

impl PolledSource for Clight {
    const INTERVAL: Duration = Duration::from_secs(10);

    fn polls(key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(name_owned(bus::Bus::Session, CLIGHT_SERVICE), move || {
            ticked_polls::<Self>(key).boxed()
        })
    }

    async fn poll((): ()) -> Result<Self, Self::Error> {
        let proxy: ConfBacklightProxy = bus::session_proxy().await?;
        Ok(Clight {
            auto: !bus::call(proxy.no_auto_calib().await)?,
        })
    }
}

fn no_auto_calib() -> Option<bool> {
    let connection = Connection::session().ok()?;
    ConfBacklightProxyBlocking::new(&connection)
        .ok()?
        .no_auto_calib()
        .ok()
}

fn set_no_auto_calib(value: bool) -> bool {
    let Ok(connection) = Connection::session() else {
        return false;
    };
    ConfBacklightProxyBlocking::new(&connection)
        .is_ok_and(|proxy| proxy.set_no_auto_calib(value).is_ok())
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

pub fn pause_auto_calib() {
    set_no_auto_calib(true);
    persist(true);
}

pub fn flip() -> bool {
    let currently_auto = no_auto_calib().is_some_and(|no| !no);
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
