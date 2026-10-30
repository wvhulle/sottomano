use std::time::Duration;

use bytesize::ByteSize;
use sottomano_sources::{
    SnapshotDelivery,
    cpu_memory_compute::{
        memory_module_temperatures::{MemoryModuleTemperature, MemoryModuleTemperatures},
        memory_usage::{Memory, MemoryProcess, MemoryProcesses, Swap},
    },
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::memory_board_group::{MemoryBoardGroup, MemoryBoardMessage, MemoryBoardTarget},
};
use uom::si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius};

use crate::{
    fixtures::{cycled, host_hardware, sample},
    showcase_message::ShowcaseMessage,
    showcased_group::ShowcasedGroup,
};

const MEMORY_USED_MEBIBYTES: &[u64] = &[21_500, 21_900, 22_300, 21_800];

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut group = MemoryBoardGroup::default();
    let _ = group.update(memory_reading(0), clock);
    let _ = group.update(
        MemoryBoardMessage::Hardware(host_hardware::ryzen_workstation_delivery()),
        clock,
    );
    ShowcasedGroup::MemoryBoard(Box::new(group))
}

pub fn live(clock: Clock) -> Vec<ShowcaseMessage> {
    let tick = sample(clock);
    [memory_reading(tick)]
        .into_iter()
        .chain(hover_fetched_snapshot_messages(
            MemoryBoardTarget::Memory,
            tick,
        ))
        .map(ShowcaseMessage::MemoryBoard)
        .collect()
}

pub fn on_hover(target: MemoryBoardTarget) -> Vec<MemoryBoardMessage> {
    hover_fetched_snapshot_messages(target, 0)
}

fn memory_reading(tick: u32) -> MemoryBoardMessage {
    MemoryBoardMessage::Memory(SnapshotDelivery::fixture_delivered(
        Ok(Memory {
            used: ByteSize::mib(cycled(MEMORY_USED_MEBIBYTES, tick)),
            total: ByteSize::gib(64),
            swap: Swap::Configured {
                used: ByteSize::mib(512),
                total: ByteSize::gib(8),
            },
        }),
        DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
    ))
}

fn hover_fetched_snapshot_messages(
    target: MemoryBoardTarget,
    tick: u32,
) -> Vec<MemoryBoardMessage> {
    match target {
        MemoryBoardTarget::Memory => vec![
            MemoryBoardMessage::MemoryProcessesFetched(SnapshotDelivery::fixture_delivered(
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
            )),
            MemoryBoardMessage::ModuleTemperaturesFetched(SnapshotDelivery::fixture_delivered(
                Ok(module_temperatures(tick)),
                DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
            )),
        ],
        MemoryBoardTarget::Board => Vec::new(),
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
