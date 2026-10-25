use bytesize::ByteSize;
use sottomano_sources::{
    SourceDelivery,
    cpu_memory_compute::memory_usage::{Memory, MemoryProcess, MemoryProcesses},
};
use sottomano_widgets::{
    clock::Clock,
    shell_message::Message,
    widget_groups::memory_board_group::{MemoryBoardGroup, MemoryBoardMessage, MemoryBoardTarget},
};

use crate::{
    fixtures::{cycled, host_hardware, sample},
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

pub fn live(clock: Clock) -> Vec<Message> {
    let tick = sample(clock);
    [memory_reading(tick)]
        .into_iter()
        .chain(on_hover_readings(MemoryBoardTarget::Memory, tick))
        .map(Message::MemoryBoard)
        .collect()
}

pub fn on_hover(target: MemoryBoardTarget) -> Vec<MemoryBoardMessage> {
    on_hover_readings(target, 0)
}

fn memory_reading(tick: u32) -> MemoryBoardMessage {
    MemoryBoardMessage::Memory(SourceDelivery::Reading(Ok(Memory {
        used: ByteSize::mib(cycled(MEMORY_USED_MEBIBYTES, tick)),
        total: ByteSize::gib(64),
    })))
}

fn on_hover_readings(target: MemoryBoardTarget, tick: u32) -> Vec<MemoryBoardMessage> {
    match target {
        MemoryBoardTarget::Memory => vec![MemoryBoardMessage::MemoryProcessesFetched(
            SourceDelivery::Reading(Ok(MemoryProcesses {
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
            })),
        )],
        MemoryBoardTarget::Board => Vec::new(),
    }
}
