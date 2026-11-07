use crate::host_machine_identity::udev_hardware_identity::{Adapter, BusId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceAttachment {
    BuiltIn,
    Usb,
}

impl From<&str> for InterfaceAttachment {
    fn from(interface: &str) -> Self {
        match Adapter::of_net_interface(interface).map(|adapter| adapter.bus_id) {
            Some(BusId::Usb { .. }) => InterfaceAttachment::Usb,
            Some(BusId::Pci { .. }) | None => InterfaceAttachment::BuiltIn,
        }
    }
}
