use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::{
        memory_growth_trend::{GrowingMemoryProcess, MemoryGrowth},
        memory_module_temperatures::{MemoryModuleTemperature, MemoryModuleTemperatures},
        memory_stall_pressure::{MemoryStallPressure, StallAverages},
        memory_usage::{Memory, MemoryProcess, MemoryProcesses},
        swap_paging_activity::{Swap, SwapActivity, SwappedProcess, SwappedProcesses},
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};
use sottomano_toml_config::severity::Severity;
use sottomano_widgets::{
    clock::Clock,
    widget_groups::memory_board_group::{MemoryBoardGroup, MemoryBoardMessage, MemoryBoardTarget},
};
use uom::si::{
    f64::{Ratio, ThermodynamicTemperature},
    ratio::percent,
    thermodynamic_temperature::degree_celsius,
};

use crate::{
    fixtures::{cycled, host_hardware, sample},
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

const MEMORY_USED_MEBIBYTES: &[u64] = &[21_500, 21_900, 22_300, 21_800];
const STALLED_LAST_MINUTE_PERCENT: &[f64] = &[0.4, 1.2, 6.5, 2.1];
const PAGED_IN_KIBIBYTES_PER_SECOND: &[u64] = &[0, 0, 18_400, 2_300];
const GROWING_TICK: u32 = 2;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = MemoryBoardGroup::default();
    for message in memory_readings(0) {
        let _ = group.update(message, clock);
    }
    let _ = group.update(
        MemoryBoardMessage::Hardware(host_hardware::ryzen_workstation_delivery()),
        clock,
    );
    ShowcasedGroup::MemoryBoard(Box::new(group))
}

pub fn live(clock: Clock) -> Vec<ShowcaseMessage> {
    let tick = sample(clock);
    memory_readings(tick)
        .into_iter()
        .chain(hover_fetched_snapshot_messages(
            &MemoryBoardTarget::Memory,
            tick,
        ))
        .map(ShowcaseMessage::MemoryBoard)
        .collect()
}

pub fn on_hover(target: &MemoryBoardTarget) -> Vec<MemoryBoardMessage> {
    hover_fetched_snapshot_messages(target, 0)
}

fn polled_every(seconds: u64) -> DeliveryCadence {
    DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(seconds)))
}

fn memory_readings(tick: u32) -> Vec<MemoryBoardMessage> {
    vec![
        MemoryBoardMessage::Memory(SnapshotDelivery::fixture_delivered(
            Ok(Memory {
                used: ByteSize::mib(cycled(MEMORY_USED_MEBIBYTES, tick)),
                total: ByteSize::gib(64),
            }),
            polled_every(5),
        )),
        MemoryBoardMessage::MemoryGrowth(SnapshotDelivery::fixture_delivered(
            Ok(memory_growth(tick)),
            polled_every(2),
        )),
        MemoryBoardMessage::StallPressure(SnapshotDelivery::fixture_delivered(
            Ok(stall_pressure(tick)),
            polled_every(2),
        )),
        MemoryBoardMessage::SwapActivity(SnapshotDelivery::fixture_delivered(
            Ok(SwapActivity {
                swap: Swap::Configured {
                    used: ByteSize::mib(512),
                    total: ByteSize::gib(8),
                },
                paged_in_per_second: ByteSize::kib(cycled(PAGED_IN_KIBIBYTES_PER_SECOND, tick)),
                paged_out_per_second: ByteSize::b(0),
            }),
            polled_every(2),
        )),
        MemoryBoardMessage::ModuleTemperatures(SnapshotDelivery::fixture_delivered(
            Ok(module_temperatures(tick)),
            polled_every(5),
        )),
    ]
}

fn memory_growth(tick: u32) -> MemoryGrowth {
    if tick % 4 != GROWING_TICK {
        return MemoryGrowth {
            used_per_minute: None,
            exhaustion_projection: None,
            filling_severity: Severity::Normal,
            growing_processes: Vec::new(),
        };
    }
    MemoryGrowth {
        used_per_minute: Some(ByteSize::mib(1_700)),
        exhaustion_projection: Some(Duration::from_secs(150)),
        filling_severity: Severity::Warning,
        growing_processes: [("cargo", 1_100), ("firefox", 420), ("rust-analyzer", 180)]
            .into_iter()
            .zip(2000..)
            .map(|((name, mebibytes_per_minute), pid)| GrowingMemoryProcess {
                pid,
                name: name.to_owned(),
                resident: ByteSize::mib(mebibytes_per_minute * 3),
                growth_per_minute: ByteSize::mib(mebibytes_per_minute),
            })
            .collect(),
    }
}

fn stall_pressure(tick: u32) -> MemoryStallPressure {
    let stalled_share =
        |scale: f64| Ratio::new::<percent>(cycled(STALLED_LAST_MINUTE_PERCENT, tick) * scale);
    let averages = |scale: f64| StallAverages {
        last_10_seconds: stalled_share(scale * 1.6),
        last_minute: stalled_share(scale),
        last_5_minutes: stalled_share(scale * 0.7),
    };
    MemoryStallPressure {
        some: averages(1.0),
        full: averages(0.8),
    }
}

fn hover_fetched_snapshot_messages(
    target: &MemoryBoardTarget,
    tick: u32,
) -> Vec<MemoryBoardMessage> {
    match target {
        MemoryBoardTarget::Memory => vec![MemoryBoardMessage::MemoryProcessesFetched(
            SnapshotDelivery::fixture_delivered(
                Ok(MemoryProcesses {
                    processes: [
                        ("firefox", 5_800 + u64::from(tick % 4) * 120),
                        ("rust-analyzer", 3_100 + u64::from(tick % 3) * 90),
                        ("sottomano", 180),
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
                DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(3))),
            ),
        )],
        MemoryBoardTarget::Swap => vec![MemoryBoardMessage::SwappedProcessesFetched(
            SnapshotDelivery::fixture_delivered(
                Ok(SwappedProcesses {
                    processes: [
                        ("firefox", 1_400 + u64::from(tick % 4) * 60),
                        ("rust-analyzer", 620 + u64::from(tick % 3) * 40),
                        ("Xwayland", 85),
                    ]
                    .into_iter()
                    .zip(3000..)
                    .map(|((name, mebibytes), pid)| SwappedProcess {
                        pid,
                        name: name.to_owned(),
                        swapped: ByteSize::mib(mebibytes),
                    })
                    .collect(),
                }),
                DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(3))),
            ),
        )],
        MemoryBoardTarget::Growth
        | MemoryBoardTarget::Pressure
        | MemoryBoardTarget::Slot(_)
        | MemoryBoardTarget::TemperatureSensor(_) => Vec::new(),
    }
}

fn module_temperatures(tick: u32) -> MemoryModuleTemperatures {
    MemoryModuleTemperatures {
        modules: [("0-0051", 41.5), ("0-0053", 43.0)]
            .into_iter()
            .map(|(bus_address, degrees)| MemoryModuleTemperature {
                bus_address: bus_address.to_owned(),
                temperature: ThermodynamicTemperature::new::<degree_celsius>(
                    degrees + f64::from(tick % 3) * 0.25,
                ),
            })
            .collect(),
    }
}
