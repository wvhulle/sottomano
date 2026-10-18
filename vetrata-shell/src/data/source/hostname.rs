use std::{str::FromStr, time::Duration};

use chrono::{DateTime, Utc};
use iced::futures::Stream;
use zbus::proxy;

use crate::{
    data::{
        empty_text::non_empty,
        source::{
            DeliveryCadence, ReadingSource, SourceDelivery,
            bus::{self, DesktopBuses},
            polled_delivery_stream,
        },
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
    },
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
    type Error = bus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "hostnamed";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_hours(1));

    fn delivery_stream(
        inputs: Self::StreamInputs,
    ) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_delivery_stream(inputs, Self::poll)
    }
}

impl OperatingSystemIdentity {
    async fn poll(buses: DesktopBuses) -> Result<Self, bus::Error> {
        let hostname = HostnameProxy::new(&buses.system).await?;
        Ok(Self {
            hostname: hostname.hostname().await?,
            kernel: hostname.kernel_release().await?,
            operating_system: hostname.operating_system_pretty_name().await?,
            chassis: hostname.chassis().await?.parse().ok(),
        })
    }
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

    #[must_use]
    pub fn board_section(&self) -> Section {
        let firmware = joined([
            self.firmware_vendor.as_deref(),
            self.firmware_version.as_deref(),
        ]);
        let firmware_date = self.firmware_date.map(|at| Cell::Time {
            at,
            format: LocalizedTimeFormat::Date,
        });
        Section::dictionary(
            [
                entry("Vendor", self.hardware_vendor.clone()),
                entry("Model", self.hardware_model.clone()),
                entry("Firmware", firmware),
                entry("Firmware date", firmware_date),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Board")
    }
}

pub fn entry(key: &'static str, value: Option<impl Into<Cell>>) -> Option<KeyValue> {
    value.map(|value| KeyValue::new(key, value))
}

fn joined<const N: usize>(parts: [Option<&str>; N]) -> Option<String> {
    non_empty(parts.into_iter().flatten().collect::<Vec<_>>().join(" "))
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

pub struct BoardFirmware<'a>(&'a MachineInfo);

impl MachineInfo {
    #[must_use]
    pub fn board_firmware(&self) -> Option<BoardFirmware<'_>> {
        (self.hardware_vendor.is_some() || self.hardware_model.is_some())
            .then_some(BoardFirmware(self))
    }
}

impl BarWidget for BoardFirmware<'_> {
    fn name(&self) -> &'static str {
        "Board"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Circuitry(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.0.hardware_model.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([self.0.board_section()])
    }
}
