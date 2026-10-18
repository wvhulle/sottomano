use std::{env, fs, time::Duration};

use futures::Stream;
use zbus::{blocking::Connection, proxy};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, backlight, bus,
    bus::{DesktopBuses, Error, name_owned},
    polled_delivery_stream,
    stream_operators::StreamOperators,
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

impl ReadingSource for Clight {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "clight";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(10));

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        name_owned(buses.session.clone(), CLIGHT_SERVICE).while_held(
            move || polled_delivery_stream(buses.clone(), Self::poll),
            SourceDelivery::Absent,
        )
    }
}

impl Clight {
    async fn poll(buses: DesktopBuses) -> Result<Self, Error> {
        let proxy: ConfBacklightProxy = buses.proxy(bus::Bus::Session).await?;
        Ok(Clight {
            auto: !proxy.no_auto_calib().await?,
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

#[must_use]
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
