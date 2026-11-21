use std::fmt::{Display, Formatter, Result as FmtResult};

use futures::{Stream, StreamExt, stream};

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    host_machine_identity::{
        hostname1_os_identity::MachineInfo,
        udev_hardware_identity::{
            CpuModel, MemorySlotModule, UdevProperties, memory_slot_modules::memory_modules,
        },
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostHardware {
    pub cpu: CpuModel,
    pub memory: Vec<MemorySlotModule>,
    pub machine: MachineInfo,
}

#[derive(Debug, Clone)]
pub enum HostHardwareError {
    HostnamedUnreadable(zbus::Error),
    Panicked,
}

impl From<zbus::Error> for HostHardwareError {
    fn from(error: zbus::Error) -> Self {
        Self::HostnamedUnreadable(error)
    }
}

impl From<Panicked> for HostHardwareError {
    fn from(Panicked: Panicked) -> Self {
        Self::Panicked
    }
}

impl Display for HostHardwareError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::HostnamedUnreadable(error) => {
                write!(formatter, "cannot read hostnamed: {error}")
            }
            Self::Panicked => write!(formatter, "hardware read panicked"),
        }
    }
}

impl SnapshotSource for HostHardware {
    type Error = HostHardwareError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Host hardware";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(Self::read(buses)).map(SnapshotDelivery::single_read)
    }
}

impl HostHardware {
    async fn read(buses: DesktopBuses) -> Result<Self, HostHardwareError> {
        let machine = MachineInfo::read(&buses.system).await?;
        blocking_poll(move || {
            Ok(HostHardware {
                cpu: CpuModel::current(),
                memory: memory_modules(&UdevProperties::dmi()),
                machine,
            })
        })
        .await
    }
}
