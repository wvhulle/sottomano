use std::{fs, str::FromStr};

use chrono::{DateTime, Utc};
use futures::Stream;
use zbus::proxy;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::ChangeTriggerStreams,
    host_machine_identity::udev_hardware_identity::host_bridge_subsystem_vendor,
    non_empty_text::non_empty,
    system_protocol_clients::{
        dbus_change_signals::{change_triggers, properties_changed_rule},
        dbus_desktop_buses::{self, DesktopBuses},
    },
};

const OS_RELEASE_PATHS: [&str; 2] = ["/etc/os-release", "/usr/lib/os-release"];
const HOSTNAME_SERVICE: &str = "org.freedesktop.hostname1";

#[proxy(
    interface = "org.freedesktop.hostname1",
    default_service = "org.freedesktop.hostname1",
    default_path = "/org/freedesktop/hostname1",
    gen_blocking = false
)]
trait Hostname {
    #[zbus(property)]
    fn hostname(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn operating_system_pretty_name(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn kernel_release(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn chassis(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn hardware_vendor(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn hardware_model(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn firmware_vendor(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn firmware_version(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn firmware_date(&self) -> zbus::Result<u64>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatingSystemIdentity {
    pub hostname: String,
    pub kernel: String,
    pub operating_system: String,
    pub distribution_ids: Vec<String>,
    pub chassis: Option<Chassis>,
}

fn os_release_value<'a>(os_release: &'a str, key: &str) -> Option<&'a str> {
    os_release.lines().find_map(|line| {
        let value = line.trim().strip_prefix(key)?.strip_prefix('=')?;
        Some(value.trim_matches(['"', '\'']))
    })
}

fn distribution_ids_in(os_release: &str) -> Vec<String> {
    let own_id = os_release_value(os_release, "ID");
    let related_ids = os_release_value(os_release, "ID_LIKE")
        .into_iter()
        .flat_map(str::split_whitespace);
    own_id
        .into_iter()
        .chain(related_ids)
        .filter(|distribution_id| !distribution_id.is_empty())
        .map(str::to_owned)
        .collect()
}

fn current_distribution_ids() -> Vec<String> {
    OS_RELEASE_PATHS
        .into_iter()
        .find_map(|path| {
            fs::read_to_string(path)
                .inspect_err(|read_failure| {
                    tracing::debug!("os identity: cannot read {path}: {read_failure}");
                })
                .ok()
        })
        .map(|os_release| distribution_ids_in(&os_release))
        .unwrap_or_default()
}

impl SnapshotSource for OperatingSystemIdentity {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Operating system";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SnapshotDelivery::live_push_deliveries(buses, identity_changes)
    }
}

async fn identity_changes(
    buses: DesktopBuses,
) -> Result<
    impl Stream<Item = Result<OperatingSystemIdentity, dbus_desktop_buses::Error>> + Send,
    dbus_desktop_buses::Error,
> {
    let hostname = HostnameProxy::new(&buses.system).await?;
    let kernel = hostname.kernel_release().await?;
    let operating_system = hostname.operating_system_pretty_name().await?;
    let distribution_ids = current_distribution_ids();
    let changes = change_triggers(
        &buses.system,
        [properties_changed_rule(HOSTNAME_SERVICE, HOSTNAME_SERVICE)?],
    )
    .await?;
    Ok(changes.triggered_rereads(move || {
        let hostname = hostname.clone();
        let kernel = kernel.clone();
        let operating_system = operating_system.clone();
        let distribution_ids = distribution_ids.clone();
        async move {
            Ok(OperatingSystemIdentity {
                hostname: hostname.hostname().await?,
                kernel,
                operating_system,
                distribution_ids,
                chassis: hostname
                    .chassis()
                    .await?
                    .parse()
                    .inspect_err(|UnknownChassis(chassis_name)| {
                        if !chassis_name.is_empty() {
                            tracing::warn!("os identity: unknown chassis {chassis_name:?}");
                        }
                    })
                    .ok(),
            })
        }
    }))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineInfo {
    pub board_pci_vendor: Option<u16>,
    pub hardware_vendor: Option<String>,
    pub hardware_model: Option<String>,
    pub firmware_vendor: Option<String>,
    pub firmware_version: Option<String>,
    pub firmware_date: Option<DateTime<Utc>>,
}

fn present_property<Value>(reply: zbus::Result<Value>, property_name: &str) -> Option<Value> {
    reply
        .inspect_err(|property_failure| {
            tracing::debug!(
                "machine info: cannot read hostname1 {property_name}: {property_failure}"
            );
        })
        .ok()
}

fn text(reply: zbus::Result<String>, property_name: &str) -> Option<String> {
    present_property(reply, property_name).and_then(non_empty)
}

pub fn at_micros(usec: u64) -> Option<DateTime<Utc>> {
    (usec > 0 && usec < u64::MAX)
        .then(|| {
            i64::try_from(usec)
                .inspect_err(|conversion_failure| {
                    tracing::warn!(
                        "timestamp {usec} microseconds out of range: {conversion_failure}"
                    );
                })
                .ok()
        })
        .flatten()
        .and_then(DateTime::from_timestamp_micros)
}

impl MachineInfo {
    pub async fn read(system: &zbus::Connection) -> zbus::Result<Self> {
        let hostname = HostnameProxy::new(system).await?;
        Ok(Self {
            board_pci_vendor: host_bridge_subsystem_vendor(),
            hardware_vendor: text(hostname.hardware_vendor().await, "HardwareVendor"),
            hardware_model: text(hostname.hardware_model().await, "HardwareModel"),
            firmware_vendor: text(hostname.firmware_vendor().await, "FirmwareVendor"),
            firmware_version: text(hostname.firmware_version().await, "FirmwareVersion"),
            firmware_date: present_property(hostname.firmware_date().await, "FirmwareDate")
                .and_then(at_micros),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chassis {
    Desktop,
    Laptop,
    Convertible,
    Server,
    Tablet,
    Handset,
    Watch,
    Embedded,
    VirtualMachine,
    Container,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownChassis(pub String);

impl FromStr for Chassis {
    type Err = UnknownChassis;

    fn from_str(chassis: &str) -> Result<Self, UnknownChassis> {
        match chassis {
            "desktop" => Ok(Chassis::Desktop),
            "laptop" => Ok(Chassis::Laptop),
            "convertible" => Ok(Chassis::Convertible),
            "server" => Ok(Chassis::Server),
            "tablet" => Ok(Chassis::Tablet),
            "handset" => Ok(Chassis::Handset),
            "watch" => Ok(Chassis::Watch),
            "embedded" => Ok(Chassis::Embedded),
            "vm" => Ok(Chassis::VirtualMachine),
            "container" => Ok(Chassis::Container),
            _ => Err(UnknownChassis(chassis.to_owned())),
        }
    }
}

impl Chassis {
    #[must_use]
    pub const fn stored_name(self) -> &'static str {
        match self {
            Chassis::Desktop => "desktop",
            Chassis::Laptop => "laptop",
            Chassis::Convertible => "convertible",
            Chassis::Server => "server",
            Chassis::Tablet => "tablet",
            Chassis::Handset => "handset",
            Chassis::Watch => "watch",
            Chassis::Embedded => "embedded",
            Chassis::VirtualMachine => "vm",
            Chassis::Container => "container",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_lists_own_id_before_related_ids() {
        let os_release =
            "NAME=\"Pop!_OS\"\nID=pop\nID_LIKE=\"ubuntu debian\"\nVERSION_ID=\"22.04\"\n";
        assert_eq!(distribution_ids_in(os_release), ["pop", "ubuntu", "debian"]);
    }

    #[test]
    fn os_release_id_ignores_longer_keys() {
        let os_release = "VERSION_ID=26.11\nID=nixos\n";
        assert_eq!(distribution_ids_in(os_release), ["nixos"]);
    }
}
