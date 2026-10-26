use sottomano_sources::{
    cpu_memory_compute::pci_gpu_readings::GpuVendor,
    host_machine_identity::udev_hardware_identity::{BusId, CpuVendor},
};

use crate::widgets::{
    bar_widget::Glyph,
    simple_icons::SimpleIcon,
    tooltip_body::{Cell, KeyValue},
};

pub trait VendorLogo {
    fn vendor_logo(&self) -> Option<SimpleIcon>;
}

impl VendorLogo for CpuVendor {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        Some(match self {
            CpuVendor::Amd => SimpleIcon::AMD,
            CpuVendor::Intel => SimpleIcon::Intel,
            CpuVendor::Arm => SimpleIcon::Arm,
            CpuVendor::Apple => SimpleIcon::Apple,
            CpuVendor::Qualcomm => SimpleIcon::Qualcomm,
            CpuVendor::HiSilicon => SimpleIcon::Huawei,
        })
    }
}

impl VendorLogo for GpuVendor {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        Some(match self {
            GpuVendor::Nvidia => SimpleIcon::NVIDIA,
            GpuVendor::Intel => SimpleIcon::Intel,
            GpuVendor::Amd => SimpleIcon::AMD,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PciVendorId(pub u16);

impl VendorLogo for PciVendorId {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        match self.0 {
            0x8086 => Some(SimpleIcon::Intel),
            0x1002 | 0x1022 => Some(SimpleIcon::AMD),
            0x10de => Some(SimpleIcon::NVIDIA),
            0x106b => Some(SimpleIcon::Apple),
            0x144d => Some(SimpleIcon::Samsung),
            0x1462 => Some(SimpleIcon::MSI),
            0x1043 => Some(SimpleIcon::ASUS),
            0x17aa => Some(SimpleIcon::Lenovo),
            0x1028 => Some(SimpleIcon::Dell),
            0x103c => Some(SimpleIcon::HP),
            0x1025 => Some(SimpleIcon::Acer),
            0xf111 => Some(SimpleIcon::Framework),
            0x1a58 => Some(SimpleIcon::Razer),
            0x2646 => Some(SimpleIcon::KingstonTechnology),
            0x1bb1 => Some(SimpleIcon::Seagate),
            0x1179 => Some(SimpleIcon::Toshiba),
            0x14c3 => Some(SimpleIcon::MediaTek),
            0x17cb | 0x168c => Some(SimpleIcon::Qualcomm),
            0x14e4 => Some(SimpleIcon::Broadcom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbVendorId(pub u16);

impl VendorLogo for UsbVendorId {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        match self.0 {
            0x8087 => Some(SimpleIcon::Intel),
            0x05ac => Some(SimpleIcon::Apple),
            0x04e8 => Some(SimpleIcon::Samsung),
            0x054c => Some(SimpleIcon::Sony),
            0x28de => Some(SimpleIcon::Valve),
            0x0db0 => Some(SimpleIcon::MSI),
            0x0b05 => Some(SimpleIcon::ASUS),
            0x17ef => Some(SimpleIcon::Lenovo),
            0x413c => Some(SimpleIcon::Dell),
            0x03f0 => Some(SimpleIcon::HP),
            0x1532 => Some(SimpleIcon::Razer),
            0x1b1c => Some(SimpleIcon::Corsair),
            0x1038 => Some(SimpleIcon::Steelseries),
            0x0951 => Some(SimpleIcon::KingstonTechnology),
            0x0e8d => Some(SimpleIcon::MediaTek),
            0x05c6 | 0x0cf3 => Some(SimpleIcon::Qualcomm),
            0x0a5c => Some(SimpleIcon::Broadcom),
            0x2357 => Some(SimpleIcon::TPLink),
            _ => None,
        }
    }
}

impl VendorLogo for BusId {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        match *self {
            BusId::Pci { vendor, .. } => PciVendorId(vendor).vendor_logo(),
            BusId::Usb { vendor, .. } => UsbVendorId(vendor).vendor_logo(),
        }
    }
}

pub struct DistributionIds<'a>(pub &'a [String]);

impl VendorLogo for DistributionIds<'_> {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        self.0
            .iter()
            .find_map(|distribution_id| distribution_logo(distribution_id))
    }
}

fn distribution_logo(distribution_id: &str) -> Option<SimpleIcon> {
    match distribution_id {
        "nixos" => Some(SimpleIcon::NixOS),
        "arch" => Some(SimpleIcon::ArchLinux),
        "fedora-asahi-remix" => Some(SimpleIcon::AsahiLinux),
        "fedora" => Some(SimpleIcon::Fedora),
        "ubuntu" => Some(SimpleIcon::Ubuntu),
        "debian" => Some(SimpleIcon::Debian),
        "opensuse" | "opensuse-tumbleweed" | "opensuse-leap" | "suse" => Some(SimpleIcon::openSUSE),
        "gentoo" => Some(SimpleIcon::Gentoo),
        "alpine" => Some(SimpleIcon::AlpineLinux),
        "void" => Some(SimpleIcon::VoidLinux),
        "linuxmint" => Some(SimpleIcon::LinuxMint),
        "pop" => Some(SimpleIcon::PopOS),
        "manjaro" => Some(SimpleIcon::Manjaro),
        "endeavouros" => Some(SimpleIcon::EndeavourOS),
        "elementary" => Some(SimpleIcon::elementary),
        "rocky" => Some(SimpleIcon::RockyLinux),
        "almalinux" => Some(SimpleIcon::AlmaLinux),
        "rhel" => Some(SimpleIcon::RedHat),
        "centos" => Some(SimpleIcon::CentOS),
        "kali" => Some(SimpleIcon::KaliLinux),
        "solus" => Some(SimpleIcon::Solus),
        "cachyos" => Some(SimpleIcon::CachyOS),
        "garuda" => Some(SimpleIcon::GarudaLinux),
        "slackware" => Some(SimpleIcon::Slackware),
        _ => None,
    }
}

pub struct PnpManufacturerName<'a>(pub &'a str);

impl VendorLogo for PnpManufacturerName<'_> {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        match self.0 {
            "Dell Inc." => Some(SimpleIcon::Dell),
            "Samsung Electric Company" | "Samsung Display Corp." => Some(SimpleIcon::Samsung),
            "LG Electronics" | "LG Display" => Some(SimpleIcon::LG),
            "Sony" => Some(SimpleIcon::Sony),
            "Lenovo Group Limited" => Some(SimpleIcon::Lenovo),
            "Hewlett Packard" | "HP Inc." => Some(SimpleIcon::HP),
            "ASUSTek COMPUTER INC" => Some(SimpleIcon::ASUS),
            "Acer Technologies" => Some(SimpleIcon::Acer),
            "Microstep" => Some(SimpleIcon::MSI),
            "Apple Computer Inc" => Some(SimpleIcon::Apple),
            "Razer Taiwan Co. Ltd." => Some(SimpleIcon::Razer),
            _ => None,
        }
    }
}

impl KeyValue {
    pub fn keyed_by_logo(
        key: &'static str,
        logo: Option<SimpleIcon>,
        value: impl Into<Cell>,
    ) -> Self {
        match logo {
            Some(vendor_logo) => KeyValue::new(Cell::Glyph(Glyph::from(vendor_logo)), value),
            None => KeyValue::new(key, value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_falls_back_to_related_ids() {
        let derivative_ids = ["zorin".to_owned(), "ubuntu".to_owned(), "debian".to_owned()];
        assert_eq!(
            DistributionIds(&derivative_ids).vendor_logo(),
            Some(SimpleIcon::Ubuntu)
        );
    }

    #[test]
    fn unknown_distribution_has_no_logo() {
        assert_eq!(DistributionIds(&["hurd".to_owned()]).vendor_logo(), None);
    }

    #[test]
    fn bus_vendor_tables_are_kept_apart() {
        assert_eq!(
            BusId::Pci {
                vendor: 0x1462,
                device: 0x7d75
            }
            .vendor_logo(),
            Some(SimpleIcon::MSI)
        );
        assert_eq!(
            BusId::Usb {
                vendor: 0x1462,
                product: 0x0001
            }
            .vendor_logo(),
            None
        );
    }
}
