use chrono::{DateTime, Utc};
use iced::{
    Subscription,
    futures::{Stream, StreamExt, stream},
};
use zbus::{Connection, proxy};

use crate::{
    data::source::bus::{self, Properties, prop, service_absent},
    message_subscriptions::restarting,
};

const UPDATABLE_DEVICE_FLAG: u64 = 1 << 1;

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

pub type PendingUpgrades = Result<Vec<FirmwareUpgrade>, bus::Error>;

pub fn subscription() -> Subscription<PendingUpgrades> {
    Subscription::run(signalled_upgrades)
}

fn signalled_upgrades() -> impl Stream<Item = PendingUpgrades> {
    restarting("fwupd change signal", connected_then_daemon_changes).then(|()| pending_upgrades())
}

async fn connected_then_daemon_changes() -> zbus::Result<impl Stream<Item = ()>> {
    let connection = Connection::system().await?;
    let fwupd = FwupdProxy::new(&connection).await?;
    Ok(stream::once(async {}).chain(stream::select_all([
        fwupd.receive_changed().await?.map(|_| ()).boxed(),
        fwupd.receive_device_added().await?.map(|_| ()).boxed(),
        fwupd.receive_device_removed().await?.map(|_| ()).boxed(),
        fwupd.receive_device_changed().await?.map(|_| ()).boxed(),
    ])))
}

async fn pending_upgrades() -> PendingUpgrades {
    let connection = Connection::system()
        .await
        .map_err(|error| bus::Error::Connect(error.to_string()))?;
    let fwupd = FwupdProxy::new(&connection)
        .await
        .map_err(|error| bus::Error::Call(error.to_string()))?;
    let devices = match bus::call(fwupd.get_devices().await) {
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
