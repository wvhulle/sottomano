use sottomano_config::widget_groups::temperature_sensors::ThresholdOverrides;
use sottomano_sources::{
    SourceDelivery,
    cpu_memory_compute::{
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
    fixtures::{cycled, sample},
    showcased_group::ShowcasedGroup,
};

const CPU_BUSY_PERCENT: &[f64] = &[37.0, 52.0, 44.0, 61.0, 48.0];
const TEMPERATURE_CELSIUS: &[f64] = &[58.0, 61.0, 63.0, 60.0];
const PACKAGE_WATTS: &[f64] = &[48.0, 63.0, 55.0, 71.0];

pub fn group(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::Compute(Box::new(continuous_readings(0).into_iter().fold(
        ComputeGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading, clock);
            group
        },
    )))
}

pub fn live(clock: Clock) -> Vec<Message> {
    let tick = sample(clock);
    continuous_readings(tick)
        .into_iter()
        .chain(on_hover_readings(ComputeTarget::Cpu, tick))
        .map(Message::Compute)
        .collect()
}

pub fn on_hover(target: ComputeTarget) -> Vec<ComputeMessage> {
    on_hover_readings(target, 0)
}

fn celsius(degrees: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(degrees)
}

fn continuous_readings(tick: u32) -> [ComputeMessage; 2] {
    [
        ComputeMessage::Cpu(SourceDelivery::Reading(Ok(CpuUsage {
            busy: Ratio::new::<percent>(cycled(CPU_BUSY_PERCENT, tick)),
        }))),
        ComputeMessage::Temperature(SourceDelivery::Reading(Ok(Temperature {
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
            vec![ComputeMessage::CpuPowerFetched(SourceDelivery::Reading(
                Ok(CpuPower {
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
                }),
            ))]
        }
        ComputeTarget::CpuClock
        | ComputeTarget::ClockLimits
        | ComputeTarget::Temperature
        | ComputeTarget::CachePreference => Vec::new(),
    }
}
