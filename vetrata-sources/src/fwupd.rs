use std::future::ready;

use chrono::{DateTime, Utc};
use iced::futures::{Stream, StreamExt, stream};
use vetrata_config::severity::Severity;
use zbus::proxy;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    bus::{self, DesktopBuses, Properties, name_owned, prop, service_absent},
    stream_operators::{StreamOperators, connected_until_failure},
};

const UPDATABLE_DEVICE_FLAG: u64 = 1 << 1;
const FWUPD_SERVICE: &str = "org.freedesktop.fwupd";

#[proxy(
    interface = "org.freedesktop.fwupd",
    default_service = "org.freedesktop.fwupd",
    default_path = "/"
)]
trait Fwupd {
    fn get_devices(&self) -> zbus::Result<Vec<Properties>>;
    fn get_upgrades(&self, device_id: &str) -> zbus::Result<Vec<Properties>>;
    #[zbus(signal)]
    fn changed(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn device_added(&self, device: Properties) -> zbus::Result<()>;
    #[zbus(signal)]
    fn device_removed(&self, device: Properties) -> zbus::Result<()>;
    #[zbus(signal)]
    fn device_changed(&self, device: Properties) -> zbus::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low,
    Medium,
    High,
    Critical,
}

impl Urgency {
    fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(Urgency::Low),
            2 => Some(Urgency::Medium),
            3 => Some(Urgency::High),
            4 => Some(Urgency::Critical),
            _ => None,
        }
    }

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Urgency::Low => "Low",
            Urgency::Medium => "Medium",
            Urgency::High => "High",
            Urgency::Critical => "Critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmwareUpgrade {
    pub device: String,
    pub vendor: Option<String>,
    pub current_version: String,
    pub offered_version: String,
    pub offered_released: Option<DateTime<Utc>>,
    pub urgency: Option<Urgency>,
    pub summary: Option<String>,
}

impl FirmwareUpgrade {
    #[must_use]
    pub fn severity(&self) -> Severity {
        match self.urgency {
            Some(Urgency::High | Urgency::Critical) => Severity::Critical,
            Some(Urgency::Low | Urgency::Medium) | None => Severity::Warning,
        }
    }
}

pub type PendingUpgrades = Result<Vec<FirmwareUpgrade>, bus::Error>;

impl ReadingSource for Vec<FirmwareUpgrade> {
    type Error = bus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "firmware upgrades";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let system = buses.system;
        name_owned(system.clone(), FWUPD_SERVICE).while_held(
            move || signalled_upgrades(system.clone()).map(SourceDelivery::Reading),
            SourceDelivery::Absent,
        )
    }
}

fn signalled_upgrades(system: zbus::Connection) -> impl Stream<Item = PendingUpgrades> + Send {
    connected_until_failure(connected_then_daemon_changes(system.clone())).then(move |changed| {
        let system = system.clone();
        async move {
            changed?;
            pending_upgrades(&system).await
        }
    })
}

async fn connected_then_daemon_changes(
    system: zbus::Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<()>>> {
    let fwupd = FwupdProxy::new(&system).await?;
    let changes = stream::select(
        stream::select(
            fwupd.receive_changed().await?.map(drop),
            fwupd.receive_device_added().await?.map(drop),
        ),
        stream::select(
            fwupd.receive_device_removed().await?.map(drop),
            fwupd.receive_device_changed().await?.map(drop),
        ),
    );
    Ok(stream::once(ready(())).chain(changes).map(Ok))
}

async fn pending_upgrades(system: &zbus::Connection) -> PendingUpgrades {
    let fwupd = FwupdProxy::new(system).await?;
    let devices = match fwupd.get_devices().await.map_err(bus::Error::from) {
        Ok(devices) => devices,
        Err(bus::Error::Absent) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut upgrades = Vec::new();
    for device in devices.iter().filter(|device| updatable(device)) {
        let Some(device_id) = prop::<String>(device, "DeviceId") else {
            continue;
        };
        let releases = match fwupd.get_upgrades(&device_id).await {
            Ok(releases) => releases,
            Err(error) if no_upgrade(&error) || service_absent(&error) => continue,
            Err(error) => return Err(bus::Error::Call(error.to_string())),
        };
        upgrades.extend(
            releases
                .first()
                .and_then(|release| FirmwareUpgrade::offered(device, release)),
        );
    }
    Ok(upgrades)
}

fn updatable(device: &Properties) -> bool {
    prop::<u64>(device, "Flags").is_some_and(|flags| flags & UPDATABLE_DEVICE_FLAG != 0)
}

fn no_upgrade(error: &zbus::Error) -> bool {
    let zbus::Error::MethodError(name, _, _) = error else {
        return false;
    };
    matches!(
        name.as_str(),
        "org.freedesktop.fwupd.NothingToDo"
            | "org.freedesktop.fwupd.NotSupported"
            | "org.freedesktop.fwupd.NotFound"
    )
}

impl FirmwareUpgrade {
    fn offered(device: &Properties, release: &Properties) -> Option<Self> {
        Some(Self {
            device: prop(device, "Name")?,
            vendor: prop(device, "Vendor"),
            current_version: prop(device, "Version")?,
            offered_version: prop(release, "Version")?,
            offered_released: prop::<u64>(release, "Created")
                .and_then(|seconds| i64::try_from(seconds).ok())
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
            urgency: prop::<u32>(release, "Urgency").and_then(Urgency::from_code),
            summary: prop(release, "Summary"),
        })
    }
}
