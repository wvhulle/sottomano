use super::{GpuNames, combined_gpu_names};

const NVML_PRODUCT: &str = "NVIDIA GeForce RTX 4090";

fn combined_with_nvml_product(database_names: Option<GpuNames>) -> GpuNames {
    combined_gpu_names(database_names, NVML_PRODUCT.to_owned())
}

#[test]
fn database_product_is_kept_over_the_branded_nvml_name() {
    let database_names = GpuNames::ChipWithProduct {
        chip: "AD102".to_owned(),
        product: "GeForce RTX 4090".to_owned(),
    };
    assert_eq!(
        combined_with_nvml_product(Some(database_names.clone())),
        database_names
    );
}

#[test]
fn database_chip_without_single_product_takes_the_nvml_name() {
    assert_eq!(
        combined_with_nvml_product(Some(GpuNames::Chip("AD102".to_owned()))),
        GpuNames::ChipWithProduct {
            chip: "AD102".to_owned(),
            product: NVML_PRODUCT.to_owned(),
        }
    );
}

#[test]
fn missing_database_entry_takes_the_nvml_name() {
    assert_eq!(
        combined_with_nvml_product(None),
        GpuNames::Product(NVML_PRODUCT.to_owned())
    );
}
