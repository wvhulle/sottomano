use chrono::{DateTime, Utc};
use zbus::{blocking, proxy};

use crate::{
    data::empty_text::non_empty,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
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
    fn pretty_hostname(&self) -> zbus::Result<String>;
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

pub fn hostname_blocking() -> Option<String> {
    let connection = blocking::Connection::system().ok()?;
    let hostname = HostnameProxyBlocking::new(&connection).ok()?;
    hostname.hostname().ok().and_then(non_empty)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineInfo {
    pub hostname: Option<String>,
    pub pretty_hostname: Option<String>,
    pub operating_system: Option<String>,
    pub kernel: Option<String>,
    pub chassis: Option<String>,
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
    pub async fn read() -> Self {
        let Ok(connection) = zbus::Connection::system().await else {
            return Self::default();
        };
        let Ok(hostname) = HostnameProxy::new(&connection).await else {
            return Self::default();
        };
        Self {
            hostname: text(hostname.hostname().await),
            pretty_hostname: text(hostname.pretty_hostname().await),
            operating_system: text(hostname.operating_system_pretty_name().await),
            kernel: text(hostname.kernel_release().await),
            chassis: text(hostname.chassis().await),
            hardware_vendor: text(hostname.hardware_vendor().await),
            hardware_model: text(hostname.hardware_model().await),
            firmware_vendor: text(hostname.firmware_vendor().await),
            firmware_version: text(hostname.firmware_version().await),
            firmware_date: hostname.firmware_date().await.ok().and_then(at_micros),
        }
    }

    pub fn section(&self) -> Section {
        let hostname = self
            .pretty_hostname
            .clone()
            .map(|pretty| match &self.hostname {
                Some(host) => format!("{pretty} ({host})"),
                None => pretty,
            })
            .or_else(|| self.hostname.clone());
        let hardware = joined([
            self.hardware_vendor.as_deref(),
            self.hardware_model.as_deref(),
        ]);
        let firmware = joined([
            self.firmware_vendor.as_deref(),
            self.firmware_version.as_deref(),
        ]);
        let firmware_date = self.firmware_date.map(|at| Cell::Time {
            at,
            part: TimePart::Date,
        });
        Section::dictionary(
            [
                entry("Hostname", hostname),
                entry("OS", self.operating_system.clone()),
                entry("Kernel", self.kernel.clone()),
                entry("Chassis", self.chassis.clone()),
                entry("Hardware", hardware),
                entry("Firmware", firmware),
                entry("Firmware date", firmware_date),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Machine")
    }
}

pub fn entry(key: &'static str, value: Option<impl Into<Cell>>) -> Option<KeyValue> {
    value.map(|value| KeyValue::new(key, value))
}

fn joined<const N: usize>(parts: [Option<&str>; N]) -> Option<String> {
    non_empty(parts.into_iter().flatten().collect::<Vec<_>>().join(" "))
}

impl BarWidget for MachineInfo {
    fn name(&self) -> &'static str {
        "Machine"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Desktop(Weight::Regular).into()
    }

    fn label(&self) -> Option<String> {
        self.hostname.clone()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([self.section()])
    }
}
