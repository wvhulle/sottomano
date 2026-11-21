use super::{GpuVendor, drm_gpu_listing::gpu_vendor_of_pci_id};
use crate::cpu_memory_compute::pci_gpu_samples::GpuNames;

#[test]
fn reads_vendor_from_udev_hex_in_either_case() {
    assert_eq!(gpu_vendor_of_pci_id("10DE"), Some(GpuVendor::Nvidia));
    assert_eq!(gpu_vendor_of_pci_id("10de"), Some(GpuVendor::Nvidia));
    assert_eq!(gpu_vendor_of_pci_id("8086"), Some(GpuVendor::Intel));
    assert_eq!(gpu_vendor_of_pci_id("1002"), Some(GpuVendor::Amd));
    assert_eq!(gpu_vendor_of_pci_id("1AF4"), None);
}

#[test]
fn splits_chip_from_single_product_name() {
    assert_eq!(
        GpuNames::split_database_name("GB206GLM [RTX PRO 2000 Blackwell Generation Laptop GPU]"),
        GpuNames::ChipWithProduct {
            chip: "GB206GLM".to_owned(),
            product: "RTX PRO 2000 Blackwell Generation Laptop GPU".to_owned(),
        }
    );
}

#[test]
fn keeps_only_chip_when_products_are_alternatives() {
    assert_eq!(
        GpuNames::split_database_name("Arrow Lake-P [Arc Pro 130T/140T]"),
        GpuNames::Chip("Arrow Lake-P".to_owned())
    );
}

#[test]
fn unbracketed_name_is_the_chip() {
    assert_eq!(
        GpuNames::split_database_name("Navi 31"),
        GpuNames::Chip("Navi 31".to_owned())
    );
}

#[test]
fn preferred_name_is_the_product_when_known() {
    assert_eq!(
        GpuNames::split_database_name("AD102 [GeForce RTX 4090]").preferred_name(),
        "GeForce RTX 4090"
    );
    assert_eq!(
        GpuNames::split_database_name("Navi 31").preferred_name(),
        "Navi 31"
    );
}
