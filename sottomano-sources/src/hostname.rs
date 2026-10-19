use std::str::FromStr;

use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt, stream};
use zbus::proxy;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    non_empty_text::non_empty,
    pushed_delivery_stream,
    system_protocol_clients::dbus_desktop_buses::{self, DesktopBuses},
};

#[proxy(
    interface = "org.freedesktop.hostname1",
    default_service = "org.freedesktop.hostname1",
    default_path = "/org/freedesktop/hostname1"
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
    pub chassis: Option<Chassis>,
}

impl ReadingSource for OperatingSystemIdentity {
    type Error = dbus_desktop_buses::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "hostnamed";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        pushed_delivery_stream(identity_changes(buses))
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
    let changes = stream::select(
        hostname.receive_hostname_changed().await.map(drop),
        hostname.receive_chassis_changed().await.map(drop),
    );
    Ok(changes.then(move |()| {
        let hostname = hostname.clone();
        let kernel = kernel.clone();
        let operating_system = operating_system.clone();
        async move {
            Ok(OperatingSystemIdentity {
                hostname: hostname.hostname().await?,
                kernel,
                operating_system,
                chassis: hostname.chassis().await?.parse().ok(),
            })
        }
    }))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineInfo {
    pub hardware_vendor: Option<String>,
    pub hardware_model: Option<String>,
    pub firmware_vendor: Option<String>,
    pub firmware_version: Option<String>,
    pub firmware_date: Option<DateTime<Utc>>,
}

fn text(reply: zbus::Result<String>) -> Option<String> {
    reply.ok().and_then(non_empty)
}

pub fn at_micros(usec: u64) -> Option<DateTime<Utc>> {
    (usec > 0 && usec < u64::MAX)
        .then(|| i64::try_from(usec).ok())
        .flatten()
        .and_then(DateTime::from_timestamp_micros)
}

impl MachineInfo {
    pub async fn read(system: &zbus::Connection) -> zbus::Result<Self> {
        let hostname = HostnameProxy::new(system).await?;
        Ok(Self {
            hardware_vendor: text(hostname.hardware_vendor().await),
            hardware_model: text(hostname.hardware_model().await),
            firmware_vendor: text(hostname.firmware_vendor().await),
            firmware_version: text(hostname.firmware_version().await),
            firmware_date: hostname.firmware_date().await.ok().and_then(at_micros),
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
