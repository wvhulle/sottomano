use std::{env, path::PathBuf};

use futures::Stream;
use zbus::{Connection, proxy, proxy::CacheProperties};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::ChangeTriggerStreams,
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
    default_path = "/org/clight/clight/Conf/Backlight",
    gen_blocking = false
)]
pub trait ConfBacklight {
    #[zbus(property)]
    fn no_auto_calib(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_no_auto_calib(&self, value: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.clight.clight",
    default_service = "org.clight.clight",
    default_path = "/org/clight/clight",
    gen_blocking = false
)]
pub trait ClightDaemon {
    fn capture(&self, reset_timer: bool, capture_only: bool) -> zbus::Result<()>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutoBrightness {
    pub calibration: BrightnessCalibration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrightnessCalibration {
    FromWebcam,
    Fixed,
}

impl BrightnessCalibration {
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            BrightnessCalibration::FromWebcam => BrightnessCalibration::Fixed,
            BrightnessCalibration::Fixed => BrightnessCalibration::FromWebcam,
        }
    }
}

impl SnapshotSource for AutoBrightness {
    type Error = Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Auto-brightness";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        name_owned(buses.session.clone(), CLIGHT_SERVICE).while_available(
            move || {
                let buses = buses.clone();
                SnapshotDelivery::live_push_deliveries(buses, calibration_changes)
            },
            || SnapshotDelivery::SourceUnavailable,
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
    Ok(config_written.triggered_rereads(move || {
        let conf = conf.clone();
        async move {
            Ok(AutoBrightness {
                calibration: if conf.no_auto_calib().await? {
                    BrightnessCalibration::Fixed
                } else {
                    BrightnessCalibration::FromWebcam
                },
            })
        }
    }))
}

pub fn config_file() -> Option<PathBuf> {
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::home_dir().map(|home| home.join(".config")))?;
    Some(config_home.join("clight.conf"))
}

pub async fn current_calibration() -> Result<BrightnessCalibration, Error> {
    let session = Connection::session().await?;
    let conf_backlight = ConfBacklightProxy::new(&session).await?;
    let no_auto_calibration = conf_backlight.no_auto_calib().await?;
    Ok(if no_auto_calibration {
        BrightnessCalibration::Fixed
    } else {
        BrightnessCalibration::FromWebcam
    })
}
