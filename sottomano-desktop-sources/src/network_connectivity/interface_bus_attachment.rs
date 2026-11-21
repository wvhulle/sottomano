use crate::host_machine_identity::udev_hardware_identity::{Adapter, BusId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceAttachment {
    BuiltIn,
    Usb,
}

impl From<&str> for InterfaceAttachment {
    fn from(interface: &str) -> Self {
        match Adapter::read_net_interface(interface).map(|adapter| adapter.bus_id) {
            Some(BusId::Usb { .. }) => InterfaceAttachment::Usb,
            Some(BusId::Pci { .. }) | None => InterfaceAttachment::BuiltIn,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AdapterIdentity {
    Identified(Adapter),
    Virtual,
}

impl AdapterIdentity {
    #[must_use]
    pub fn net_interface(interface: &str) -> Self {
        Adapter::read_net_interface(interface).map_or(Self::Virtual, Self::Identified)
    }

    #[must_use]
    pub fn sysfs_device(device: &str) -> Self {
        Adapter::read_sysfs_device(device).map_or(Self::Virtual, Self::Identified)
    }
}
