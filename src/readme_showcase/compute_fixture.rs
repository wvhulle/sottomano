use bytesize::ByteSize;
use uom::si::{
    f64::{Power, Ratio, ThermodynamicTemperature},
    power::watt,
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use crate::{
    config::widget_groups::temperature_sensors::ThresholdOverrides,
    data::source::{
        Delivery,
        sysfs_sensors::{
            cpu_power::{CpuPower, PowerZone},
            cpu_usage::CpuUsage,
            memory::{Memory, MemoryProcess, MemoryProcesses},
            temperature::{TempReading, Temperature},
        },
    },
    readme_showcase::showcased_group::ShowcasedGroup,
    widget_groups::compute_group::{ComputeGroup, ComputeMessage, ComputeTarget},
};

pub fn compute_group() -> ShowcasedGroup {
    ShowcasedGroup::Compute(continuous_readings().into_iter().fold(
        ComputeGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading);
            group
        },
    ))
}

fn continuous_readings() -> [ComputeMessage; 3] {
    [
        ComputeMessage::Cpu(Delivery::Reading(Ok(CpuUsage {
            busy: Ratio::new::<percent>(37.0),
        }))),
        ComputeMessage::Memory(Delivery::Reading(Ok(Memory {
            used: ByteSize::gib(21),
            total: ByteSize::gib(64),
        }))),
        ComputeMessage::Temperature(Delivery::Reading(Ok(Temperature {
            readings: vec![TempReading {
                chip: "k10temp".to_owned(),
                label: "Tctl".to_owned(),
                temperature: ThermodynamicTemperature::new::<degree_celsius>(58.0),
                limit: Some(ThermodynamicTemperature::new::<degree_celsius>(95.0)),
            }],
            throttle_limit: Some(ThermodynamicTemperature::new::<degree_celsius>(95.0)),
            overrides: ThresholdOverrides::default(),
            critical_since: None,
        }))),
    ]
}

pub fn on_hover_readings(target: ComputeTarget) -> Vec<ComputeMessage> {
    match target {
        ComputeTarget::Cpu => vec![ComputeMessage::CpuPowerFetched(Delivery::Reading(Ok(
            CpuPower {
                zones: vec![
                    PowerZone {
                        name: "package".to_owned(),
                        draw: Power::new::<watt>(48.0),
                    },
                    PowerZone {
                        name: "core".to_owned(),
                        draw: Power::new::<watt>(31.0),
                    },
                ],
            },
        )))],
        ComputeTarget::Memory => vec![ComputeMessage::MemoryProcessesFetched(Delivery::Reading(
            Ok(MemoryProcesses {
                processes: [
                    ("firefox", 5_800),
                    ("rust-analyzer", 3_100),
                    ("vetrata", 180),
                ]
                .into_iter()
                .zip(1000..)
                .map(|((name, mebibytes), pid)| MemoryProcess {
                    pid,
                    name: name.to_owned(),
                    resident: ByteSize::mib(mebibytes),
                })
                .collect(),
            }),
        ))],
        ComputeTarget::CpuClock
        | ComputeTarget::Temperature
        | ComputeTarget::CachePreference
        | ComputeTarget::Board => Vec::new(),
    }
}
