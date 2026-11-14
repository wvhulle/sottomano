use std::time::SystemTime;

use bytesize::ByteSize;
use sottomano_desktop_sources::system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad;

use crate::{
    clock::Clock,
    group_items::reading_display_state::ReadingDisplayState,
    widgets::{
        localized_quantities::Quantity,
        value_cells::{LabeledValue, ValueCell},
    },
};

#[must_use]
pub(crate) fn cpu_share_quantity(cpu_load: ProcessTreeCpuLoad) -> Option<Quantity> {
    match cpu_load {
        ProcessTreeCpuLoad::AwaitingSecondSample => None,
        ProcessTreeCpuLoad::BusyShare(share) => Some(Quantity::share(share)),
    }
}

#[must_use]
pub(crate) fn approximate_memory_text(resident_memory: ByteSize) -> String {
    format!("~{resident_memory}")
}

#[must_use]
pub(crate) fn running_state_text(running: bool) -> &'static str {
    if running { "running" } else { "stopped" }
}

#[must_use]
pub(crate) fn running_display_state(running: bool) -> ReadingDisplayState {
    if running {
        ReadingDisplayState::On
    } else {
        ReadingDisplayState::Off
    }
}

#[must_use]
pub(crate) fn uptime_entry(clock: Clock, started_at: SystemTime) -> Option<LabeledValue> {
    let Ok(uptime) = clock.wall.duration_since(started_at) else {
        return None;
    };
    Some(LabeledValue::new("Uptime", ValueCell::duration(uptime)))
}
