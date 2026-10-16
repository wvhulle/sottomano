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
    desktop_shell::Message,
    readme_showcase::{fixtures::cycled, showcased_group::ShowcasedGroup},
    widget_groups::compute_group::{ComputeGroup, ComputeMessage, ComputeTarget},
};

const CPU_BUSY_PERCENT: &[f64] = &[37.0, 52.0, 44.0, 61.0, 48.0];
const MEMORY_USED_MEBIBYTES: &[u64] = &[21_500, 21_900, 22_300, 21_800];
const TEMPERATURE_CELSIUS: &[f64] = &[58.0, 61.0, 63.0, 60.0];
const PACKAGE_WATTS: &[f64] = &[48.0, 63.0, 55.0, 71.0];

pub fn group() -> ShowcasedGroup {
    ShowcasedGroup::Compute(Box::new(continuous_readings(0).into_iter().fold(
        ComputeGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading);
            group
        },
    )))
}

pub fn live(tick: u32) -> Vec<Message> {
    continuous_readings(tick)
        .into_iter()
        .chain(on_hover_readings(ComputeTarget::Cpu, tick))
        .chain(on_hover_readings(ComputeTarget::Memory, tick))
        .map(Message::Compute)
        .collect()
}

pub fn on_hover(target: ComputeTarget) -> Vec<ComputeMessage> {
    on_hover_readings(target, 0)
}

fn celsius(degrees: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(degrees)
}

fn continuous_readings(tick: u32) -> [ComputeMessage; 3] {
    [
        ComputeMessage::Cpu(Delivery::Reading(Ok(CpuUsage {
            busy: Ratio::new::<percent>(cycled(CPU_BUSY_PERCENT, tick)),
        }))),
        ComputeMessage::Memory(Delivery::Reading(Ok(Memory {
            used: ByteSize::mib(cycled(MEMORY_USED_MEBIBYTES, tick)),
            total: ByteSize::gib(64),
        }))),
        ComputeMessage::Temperature(Delivery::Reading(Ok(Temperature {
            readings: vec![TempReading {
                chip: "k10temp".to_owned(),
                label: "Tctl".to_owned(),
                temperature: celsius(cycled(TEMPERATURE_CELSIUS, tick)),
                limit: Some(celsius(95.0)),
            }],
            throttle_limit: Some(celsius(95.0)),
            overrides: ThresholdOverrides::default(),
            critical_since: None,
        }))),
    ]
}

fn on_hover_readings(target: ComputeTarget, tick: u32) -> Vec<ComputeMessage> {
    match target {
        ComputeTarget::Cpu => {
            let package = cycled(PACKAGE_WATTS, tick);
            vec![ComputeMessage::CpuPowerFetched(Delivery::Reading(Ok(
                CpuPower {
                    zones: vec![
                        PowerZone {
                            name: "package".to_owned(),
                            draw: Power::new::<watt>(package),
                        },
                        PowerZone {
                            name: "core".to_owned(),
                            draw: Power::new::<watt>(package * 0.65),
                        },
                    ],
                },
            )))]
        }
        ComputeTarget::Memory => vec![ComputeMessage::MemoryProcessesFetched(Delivery::Reading(
            Ok(MemoryProcesses {
                processes: [
                    ("firefox", 5_800 + u64::from(tick % 4) * 120),
                    ("rust-analyzer", 3_100 + u64::from(tick % 3) * 90),
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
