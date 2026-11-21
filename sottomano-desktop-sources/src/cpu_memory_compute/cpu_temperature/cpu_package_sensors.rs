use crate::cpu_memory_compute::hwmon_temperature_sensors::{
    ChosenSensors, HwmonChip, TemperatureSensor,
};

const CPU_PACKAGE_CHIPS: [&str; 3] = ["coretemp", "k10temp", "zenpower"];
const SOC_THERMAL_ZONE_MARKERS: [&str; 2] = ["cpu", "soc"];
const PACKAGE_FALLBACK_LABELS: [&str; 3] = ["Tdie", "Tctl", "Package id 0"];

pub(super) fn detected_cpu_sensors() -> Vec<ChosenSensors> {
    let chips = HwmonChip::all();
    let package_sensors: Vec<ChosenSensors> = chips
        .iter()
        .filter(|chip| CpuChipRole::from(*chip) == CpuChipRole::CpuPackage)
        .map(package_chip_sensors)
        .filter(|chosen| !chosen.sensors.is_empty())
        .collect();
    if !package_sensors.is_empty() {
        return package_sensors;
    }
    chips
        .into_iter()
        .filter(|chip| CpuChipRole::from(chip) == CpuChipRole::SocThermalZone)
        .map(|chip| ChosenSensors {
            sensors: chip.temperature_sensors(),
            chip,
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CpuChipRole {
    CpuPackage,
    SocThermalZone,
    UnrelatedChip,
}

impl From<&HwmonChip> for CpuChipRole {
    fn from(chip: &HwmonChip) -> Self {
        if CPU_PACKAGE_CHIPS
            .iter()
            .any(|prefix| chip.name.starts_with(prefix))
        {
            CpuChipRole::CpuPackage
        } else if SOC_THERMAL_ZONE_MARKERS
            .iter()
            .any(|marker| chip.name.contains(marker))
        {
            CpuChipRole::SocThermalZone
        } else {
            CpuChipRole::UnrelatedChip
        }
    }
}

fn package_chip_sensors(chip: &HwmonChip) -> ChosenSensors {
    let sensors = chip.temperature_sensors();
    let core_prefix = if chip.name.starts_with("coretemp") {
        "Core "
    } else {
        "Tccd"
    };
    let per_core: Vec<TemperatureSensor> = sensors
        .iter()
        .filter(|sensor| sensor.label.starts_with(core_prefix))
        .cloned()
        .collect();
    let chosen = if per_core.is_empty() {
        PACKAGE_FALLBACK_LABELS
            .iter()
            .find_map(|label| sensors.iter().find(|sensor| &sensor.label == label))
            .into_iter()
            .cloned()
            .collect()
    } else {
        per_core
    };
    ChosenSensors {
        chip: chip.clone(),
        sensors: chosen,
    }
}
