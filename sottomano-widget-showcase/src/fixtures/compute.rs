use sottomano_config::widget_groups::temperature_sensors::ThresholdOverrides;
use sottomano_sources::{
    SourceDelivery,
    cpu_memory_compute::{
        cpu_busy_processes::{CpuBusyProcess, CpuBusyProcesses},
        cpu_core_loads::CpuUsage,
        cpu_power_draw::{CpuPower, PowerZone},
        cpu_temperature::{TempReading, Temperature},
    },
};
use sottomano_widgets::{
    clock::Clock,
    shell_message::Message,
    widget_groups::compute_group::{ComputeGroup, ComputeMessage, ComputeTarget},
};
use uom::si::{
    f64::{Power, Ratio, ThermodynamicTemperature},
    power::watt,
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use crate::{
    fixtures::{cycled, host_hardware, sample},
    showcased_group::ShowcasedGroup,
};

const CPU_BUSY_PERCENT: &[f64] = &[37.0, 52.0, 44.0, 61.0, 48.0];
const TEMPERATURE_CELSIUS: &[f64] = &[58.0, 61.0, 63.0, 60.0];
const PACKAGE_WATTS: &[f64] = &[48.0, 63.0, 55.0, 71.0];

pub fn group(clock: Clock) -> ShowcasedGroup {
    let hardware_identity =
        ComputeMessage::HostHardware(host_hardware::ryzen_workstation_delivery());
    ShowcasedGroup::Compute(Box::new(
        continuous_readings(0)
            .into_iter()
            .chain([hardware_identity])
            .fold(ComputeGroup::default(), |mut group, reading| {
                let _ = group.update(reading, clock);
                group
            }),
    ))
}

pub fn live(clock: Clock) -> Vec<Message> {
    let tick = sample(clock);
    continuous_readings(tick)
        .into_iter()
        .chain(on_hover_readings(ComputeTarget::CpuLoad, tick))
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
    let package = cycled(PACKAGE_WATTS, tick);
    [
        ComputeMessage::CpuLoad(SourceDelivery::Reading(Ok(CpuUsage {
            busy: Ratio::new::<percent>(cycled(CPU_BUSY_PERCENT, tick)),
        }))),
        ComputeMessage::CpuTemperature(SourceDelivery::Reading(Ok(Temperature {
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
        ComputeMessage::CpuPowerDraw(SourceDelivery::Reading(Ok(CpuPower {
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
        }))),
    ]
}

fn on_hover_readings(target: ComputeTarget, tick: u32) -> Vec<ComputeMessage> {
    match target {
        ComputeTarget::CpuLoad => {
            let busy = cycled(CPU_BUSY_PERCENT, tick);
            vec![ComputeMessage::CpuBusyProcessesFetched(
                SourceDelivery::Reading(Ok(CpuBusyProcesses {
                    processes: [("cargo", 0.5), ("rust-analyzer", 0.3), ("firefox", 0.15)]
                        .into_iter()
                        .zip(1000..)
                        .map(|((name, share), pid)| CpuBusyProcess {
                            pid,
                            name: name.to_owned(),
                            busy: Ratio::new::<percent>(busy * share),
                        })
                        .collect(),
                })),
            )]
        }
        ComputeTarget::CpuClock
        | ComputeTarget::CpuClockLimits
        | ComputeTarget::CpuTemperature
        | ComputeTarget::CpuPowerDraw
        | ComputeTarget::CpuModel
        | ComputeTarget::X3dCachePreference => Vec::new(),
    }
}
