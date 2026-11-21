use sottomano_desktop_sources::{
    cpu_memory_compute::pci_gpu_samples::GpuVendor,
    host_machine_identity::udev_hardware_identity::{BusId, CpuVendor},
};

use crate::{
    glyph_icons::{icon_glyph::IconGlyph, simple_icons::SimpleIcon},
    widgets::value_cells::{LabeledValue, ValueCell},
};

pub trait VendorLogo {
    fn vendor_logo(&self) -> Option<SimpleIcon>;
}

impl VendorLogo for CpuVendor {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        match self {
            CpuVendor::Amd => Some(SimpleIcon::AMD),
            CpuVendor::Intel => Some(SimpleIcon::Intel),
            CpuVendor::Arm => Some(SimpleIcon::Arm),
            CpuVendor::Apple => Some(SimpleIcon::Apple),
            CpuVendor::Qualcomm => Some(SimpleIcon::Qualcomm),
            CpuVendor::HiSilicon => Some(SimpleIcon::Huawei),
            CpuVendor::Unrecognised => None,
        }
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
pub struct PciVendorId {
    pub vendor_code: u16,
}

impl VendorLogo for PciVendorId {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        match self.vendor_code {
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
pub struct UsbVendorId {
    pub vendor_code: u16,
}

impl VendorLogo for UsbVendorId {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        match self.vendor_code {
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
            BusId::Pci { vendor, .. } => PciVendorId {
                vendor_code: vendor,
            }
            .vendor_logo(),
            BusId::Usb { vendor, .. } => UsbVendorId {
                vendor_code: vendor,
            }
            .vendor_logo(),
        }
    }
}

pub struct DistributionIds<'a> {
    pub distribution_names: &'a [String],
}

impl VendorLogo for DistributionIds<'_> {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        self.distribution_names
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

pub struct PnpManufacturerName<'a> {
    pub manufacturer_name: &'a str,
}

impl VendorLogo for PnpManufacturerName<'_> {
    fn vendor_logo(&self) -> Option<SimpleIcon> {
        match self.manufacturer_name {
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

impl LabeledValue {
    #[must_use]
    pub fn logo_prefixed(key: &'static str, logo: Option<SimpleIcon>, text: &str) -> Self {
        match logo {
            Some(vendor_logo) => LabeledValue::new(
                key,
                ValueCell::BrandedText {
                    text: text.to_owned(),
                    glyph: IconGlyph::from(vendor_logo),
                },
            ),
            None => LabeledValue::new(key, text),
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
            DistributionIds {
                distribution_names: &derivative_ids
            }
            .vendor_logo(),
            Some(SimpleIcon::Ubuntu)
        );
    }

    #[test]
    fn unknown_distribution_has_no_logo() {
        assert_eq!(
            DistributionIds {
                distribution_names: &["hurd".to_owned()]
            }
            .vendor_logo(),
            None
        );
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
