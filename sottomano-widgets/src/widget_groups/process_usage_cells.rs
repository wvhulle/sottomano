use bytesize::ByteSize;
use sottomano_desktop_sources::system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad;

use crate::widgets::localized_quantities::Quantity;

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
