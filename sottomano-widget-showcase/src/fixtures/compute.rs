use std::time::Duration;

use sottomano_config::widget_groups::temperature_sensors::ThresholdOverrides;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::{
        amd_l3_pmu_cache_traffic::{L3CacheTraffic, SharedCacheTraffic},
        amd_x3d_cache_preference::X3dCachePreference,
        cpu_busy_processes::{CpuBusyProcess, CpuBusyProcesses},
        cpu_clock_frequency::{CoreFreq, CpuClock, FrequencyBand},
        cpu_core_deep_idle::CoreDeepIdle,
        cpu_core_loads::{CoreLoad, CpuCoreLoads, CpuUsage, LoadSplit},
        cpu_power_draw::{CpuPower, PowerZone},
        cpu_temperature::{CoreTemperature, CpuCoreTemperatures, TempReading, Temperature},
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};
use sottomano_widgets::{
    clock::Clock,
    group_items::toggle_control::ToggleMessage,
    widget_groups::compute_group::{ComputeGroup, ComputeMessage, ComputeTarget},
};
use uom::si::{
    f64::{Frequency, Power, Ratio, ThermodynamicTemperature},
    frequency::megahertz,
    power::watt,
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use crate::{
    fixtures::{
        cycled,
        host_hardware::{
            self, CORES_PER_CHIPLET, HIGH_CLOCK_CHIPLET, PHYSICAL_CORE_COUNT, chiplet_of,
            dual_chiplet_topology, logical_cpus,
        },
        sample,
    },
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

const CPU_BUSY_PERCENT: &[f64] = &[37.0, 52.0, 44.0, 61.0, 48.0];
const TEMPERATURE_CELSIUS: &[f64] = &[58.0, 61.0, 63.0, 60.0];
const PACKAGE_WATTS: &[f64] = &[48.0, 63.0, 55.0, 71.0];
const CHIPLET_PEAK_MEGAHERTZ: [f64; 2] = [5250.0, 5650.0];
const CLOCK_FLOOR_MEGAHERTZ: f64 = 545.0;
const CHIPLET_PREFERENCE_RANKS: [u32; 2] = [236, 171];
const CHIPLET_LOOKUPS_PER_SECOND: [f64; 2] = [182.0e6, 94.0e6];
const CHIPLET_HIT_PERCENT: [f64; 2] = [71.0, 46.0];
const TEMPERATURE_LIMIT_CELSIUS: f64 = 95.0;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let hardware_identity =
        ComputeMessage::HostHardware(host_hardware::ryzen_workstation_delivery());
    let cache_preference = ComputeMessage::X3dCachePreference(ToggleMessage::Reading(
        SnapshotDelivery::live_push(Ok(X3dCachePreference::Cache)),
    ));
    ShowcasedGroup::Compute(Box::new(
        latest_snapshot_messages(0)
            .into_iter()
            .chain([hardware_identity, cache_preference])
            .fold(ComputeGroup::default(), |mut group, reading| {
                let _ = group.update(reading, clock);
                group
            }),
    ))
}

pub fn live(clock: Clock) -> Vec<ShowcaseMessage> {
    let tick = sample(clock);
    latest_snapshot_messages(tick)
        .into_iter()
        .chain(hover_fetched_snapshot_messages(
            ComputeTarget::CpuLoad,
            tick,
        ))
        .map(ShowcaseMessage::Compute)
        .collect()
}

pub fn on_hover(target: ComputeTarget) -> Vec<ComputeMessage> {
    hover_fetched_snapshot_messages(target, 0)
}

fn celsius(degrees: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(degrees)
}

fn latest_snapshot_messages(tick: u32) -> [ComputeMessage; 4] {
    let package = cycled(PACKAGE_WATTS, tick);
    [
        ComputeMessage::CpuClock(SnapshotDelivery::fixture_delivered(
            Ok(cpu_clock(tick)),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(2))),
        )),
        ComputeMessage::CpuLoad(SnapshotDelivery::fixture_delivered(
            Ok(CpuUsage {
                busy: Ratio::new::<percent>(cycled(CPU_BUSY_PERCENT, tick)),
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(3))),
        )),
        ComputeMessage::CpuTemperature(SnapshotDelivery::fixture_delivered(
            Ok(Temperature {
                readings: ["Tctl", "Tccd1", "Tccd2"]
                    .into_iter()
                    .zip([0.0, -6.0, -3.0])
                    .map(|(label, offset)| TempReading {
                        chip: "k10temp".to_owned(),
                        label: label.to_owned(),
                        temperature: celsius(cycled(TEMPERATURE_CELSIUS, tick) + offset),
                        limit: Some(celsius(TEMPERATURE_LIMIT_CELSIUS)),
                    })
                    .collect(),
                throttle_limit: Some(celsius(TEMPERATURE_LIMIT_CELSIUS)),
                overrides: ThresholdOverrides::default(),
                critical_since: None,
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        ComputeMessage::CpuPowerDraw(SnapshotDelivery::fixture_delivered(
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
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(3))),
        )),
    ]
}

fn hover_fetched_snapshot_messages(target: ComputeTarget, tick: u32) -> Vec<ComputeMessage> {
    match target {
        ComputeTarget::CpuLoad => {
            let busy = cycled(CPU_BUSY_PERCENT, tick);
            vec![
                ComputeMessage::CpuBusyProcessesFetched(SnapshotDelivery::fixture_delivered(
                    Ok(CpuBusyProcesses {
                        processes: [("cargo", 0.5), ("rust-analyzer", 0.3), ("firefox", 0.15)]
                            .into_iter()
                            .zip(1000..)
                            .map(|((name, share), pid)| CpuBusyProcess {
                                pid,
                                name: name.to_owned(),
                                busy: Ratio::new::<percent>(busy * share),
                            })
                            .collect(),
                    }),
                    every_seconds(3),
                )),
                core_loads_fetched(tick),
                ComputeMessage::CacheTrafficFetched(SnapshotDelivery::fixture_delivered(
                    Ok(cache_traffic(tick)),
                    every_seconds(1),
                )),
            ]
        }
        ComputeTarget::CpuClock => vec![core_loads_fetched(tick)],
        ComputeTarget::CpuTemperature => vec![ComputeMessage::CoreTemperaturesFetched(
            SnapshotDelivery::fixture_delivered(Ok(core_temperatures(tick)), every_seconds(2)),
        )],
        ComputeTarget::CpuClockLimits
        | ComputeTarget::CpuPowerDraw
        | ComputeTarget::CpuModel
        | ComputeTarget::X3dCachePreference => Vec::new(),
    }
}

fn every_seconds(seconds: u64) -> DeliveryCadence {
    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(seconds)))
}

fn physical_core_of(logical_cpu: u32) -> u32 {
    logical_cpu % PHYSICAL_CORE_COUNT
}

fn thread_busy_percent(logical_cpu: u32, tick: u32) -> f64 {
    let sibling_offset = if logical_cpu >= PHYSICAL_CORE_COUNT {
        9
    } else {
        0
    };
    let wave = (physical_core_of(logical_cpu) * 17 + tick * 11 + sibling_offset) % 70;
    let chiplet_bias = if chiplet_of(logical_cpu) == HIGH_CLOCK_CHIPLET {
        -12.0
    } else {
        8.0
    };
    (f64::from(wave) + chiplet_bias).clamp(2.0, 96.0)
}

fn core_loads_fetched(tick: u32) -> ComputeMessage {
    ComputeMessage::CoreLoadsFetched(SnapshotDelivery::fixture_delivered(
        Ok(core_loads(tick)),
        every_seconds(1),
    ))
}

fn core_loads(tick: u32) -> CpuCoreLoads {
    CpuCoreLoads {
        cores: logical_cpus()
            .map(|logical_cpu| {
                let busy = thread_busy_percent(logical_cpu, tick);
                CoreLoad {
                    core: logical_cpu,
                    busy: Ratio::new::<percent>(busy),
                    split: LoadSplit {
                        user: Ratio::new::<percent>(busy * 0.78),
                        system: Ratio::new::<percent>(busy * 0.17),
                        iowait: Ratio::new::<percent>(busy * 0.03),
                        irq: Ratio::new::<percent>(busy * 0.02),
                    },
                }
            })
            .collect(),
        deep_idle: logical_cpus()
            .map(|logical_cpu| CoreDeepIdle {
                core: logical_cpu,
                share: Ratio::new::<percent>(100.0 - thread_busy_percent(logical_cpu, tick) - 4.0),
            })
            .collect(),
        topology: dual_chiplet_topology(),
    }
}

fn cpu_clock(tick: u32) -> CpuClock {
    CpuClock {
        cores: logical_cpus()
            .map(|logical_cpu| {
                let chiplet = chiplet_of(logical_cpu);
                let peak = CHIPLET_PEAK_MEGAHERTZ[chiplet];
                let busy_share = thread_busy_percent(physical_core_of(logical_cpu), tick) / 100.0;
                let position_in_chiplet = physical_core_of(logical_cpu) % CORES_PER_CHIPLET;
                CoreFreq {
                    core: logical_cpu,
                    frequency: Frequency::new::<megahertz>(
                        (peak * (0.62 + 0.36 * busy_share)).round(),
                    ),
                    band: FrequencyBand {
                        floor: Frequency::new::<megahertz>(CLOCK_FLOOR_MEGAHERTZ),
                        policy_ceiling: Frequency::new::<megahertz>(peak),
                        hardware_ceiling: Frequency::new::<megahertz>(peak),
                    },
                    preference_rank: Some(CHIPLET_PREFERENCE_RANKS[chiplet] - position_in_chiplet),
                }
            })
            .collect(),
        limit_causes: Vec::new(),
        topology: dual_chiplet_topology(),
    }
}

fn core_temperatures(tick: u32) -> CpuCoreTemperatures {
    let package = cycled(TEMPERATURE_CELSIUS, tick);
    CpuCoreTemperatures {
        cores: logical_cpus()
            .map(|logical_cpu| {
                let physical_core = physical_core_of(logical_cpu);
                let chiplet_offset = if chiplet_of(logical_cpu) == HIGH_CLOCK_CHIPLET {
                    -2.0
                } else {
                    -7.0
                };
                let core_offset = f64::from((physical_core * 5 + tick) % 6);
                CoreTemperature {
                    core: logical_cpu,
                    temperature: celsius(package + chiplet_offset + core_offset),
                    limit: Some(celsius(TEMPERATURE_LIMIT_CELSIUS)),
                    overrides: ThresholdOverrides::default(),
                }
            })
            .collect(),
        topology: dual_chiplet_topology(),
    }
}

fn cache_traffic(tick: u32) -> L3CacheTraffic {
    let swing = f64::from(tick % 4) * 0.06;
    L3CacheTraffic {
        caches: [0, CORES_PER_CHIPLET]
            .into_iter()
            .zip(
                CHIPLET_LOOKUPS_PER_SECOND
                    .into_iter()
                    .zip(CHIPLET_HIT_PERCENT),
            )
            .map(
                |(counted_on_cpu, (lookups_per_second, hit_percent))| SharedCacheTraffic {
                    counted_on_cpu,
                    lookups_per_second: lookups_per_second * (1.0 + swing),
                    hit_rate: Ratio::new::<percent>(hit_percent - swing * 20.0),
                },
            )
            .collect(),
    }
}
