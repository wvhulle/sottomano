use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, stream};
use sottomano_toml_config::severity::Severity;
use zbus::proxy;

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::ChangeTriggerStreams,
    system_protocol_clients::{
        dbus_desktop_buses::{
            self, DesktopBuses, Properties, SERVICE_ABSENT_ERRORS, name_owned, prop,
        },
        protocol_peer_reconnects::subscribed,
    },
};

const UPDATABLE_DEVICE_FLAG: u64 = 1 << 1;
const FWUPD_SERVICE: &str = "org.freedesktop.fwupd";

#[proxy(
    interface = "org.freedesktop.fwupd",
    default_service = "org.freedesktop.fwupd",
    default_path = "/",
    gen_blocking = false
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

impl SnapshotSource for Vec<FirmwareUpgrade> {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Firmware upgrades";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        name_owned(system.clone(), FWUPD_SERVICE).while_available(
            move || signalled_upgrades(system.clone()).map(SnapshotDelivery::live_push),
            || SnapshotDelivery::SourceUnavailable,
        )
    }
}

fn signalled_upgrades(
    system: zbus::Connection,
) -> impl Stream<Item = Result<Vec<FirmwareUpgrade>, dbus_desktop_buses::Error>> + Send {
    subscribed(system, upgrades_on_daemon_changes)
}

async fn upgrades_on_daemon_changes(
    system: zbus::Connection,
) -> Result<
    impl Stream<Item = Result<Vec<FirmwareUpgrade>, dbus_desktop_buses::Error>> + Send,
    dbus_desktop_buses::Error,
> {
    let fwupd = FwupdProxy::new(&system).await?;
    let changes = stream::select(
        stream::select(
            fwupd
                .receive_changed()
                .await?
                .map(|_| Ok::<(), zbus::Error>(())),
            fwupd.receive_device_added().await?.map(|_| Ok(())),
        ),
        stream::select(
            fwupd.receive_device_removed().await?.map(|_| Ok(())),
            fwupd.receive_device_changed().await?.map(|_| Ok(())),
        ),
    );
    Ok(changes.triggered_rereads(move || {
        let system = system.clone();
        async move { pending_upgrades(&system).await }
    }))
}

async fn pending_upgrades(
    system: &zbus::Connection,
) -> Result<Vec<FirmwareUpgrade>, dbus_desktop_buses::Error> {
    let fwupd = FwupdProxy::new(system).await?;
    let devices = match fwupd
        .get_devices()
        .await
        .map_err(dbus_desktop_buses::Error::from)
    {
        Ok(devices) => devices,
        Err(dbus_desktop_buses::Error::Absent) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut upgrades = Vec::new();
    for device in devices.iter().filter(|device| {
        prop::<u64>(device, "Flags").is_some_and(|flags| flags & UPDATABLE_DEVICE_FLAG != 0)
    }) {
        let Some(device_id) = prop::<String>(device, "DeviceId") else {
            continue;
        };
        let releases = match fwupd.get_upgrades(&device_id).await {
            Ok(releases) => releases,
            Err(zbus::Error::MethodError(name, _, _))
                if NO_UPGRADE_ERRORS.contains(&name.as_str())
                    || SERVICE_ABSENT_ERRORS.contains(&name.as_str()) =>
            {
                continue;
            }
            Err(error) => return Err(dbus_desktop_buses::Error::Call(error.to_string())),
        };
        upgrades.extend(
            releases
                .first()
                .and_then(|release| FirmwareUpgrade::offered(device, release)),
        );
    }
    Ok(upgrades)
}

const NO_UPGRADE_ERRORS: [&str; 3] = [
    "org.freedesktop.fwupd.NothingToDo",
    "org.freedesktop.fwupd.NotSupported",
    "org.freedesktop.fwupd.NotFound",
];

impl FirmwareUpgrade {
    fn offered(device: &Properties, release: &Properties) -> Option<Self> {
        Some(Self {
            device: prop(device, "Name")?,
            vendor: prop(device, "Vendor"),
            current_version: prop(device, "Version")?,
            offered_version: prop(release, "Version")?,
            offered_released: prop::<u64>(release, "Created")
                .and_then(|seconds| {
                    i64::try_from(seconds)
                        .inspect_err(|conversion_failure| {
                            tracing::warn!(
                                "firmware upgrades: release time {seconds} out of range: \
                                 {conversion_failure}"
                            );
                        })
                        .ok()
                })
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
            urgency: prop::<u32>(release, "Urgency").and_then(Urgency::from_code),
            summary: prop(release, "Summary"),
        })
    }
}
