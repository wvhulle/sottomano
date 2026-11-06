use bytesize::ByteSize;
use sottomano_desktop_sources::system_protocol_clients::procfs_process_tree::ProcessTreeCpuLoad;

#[must_use]
pub(crate) fn cpu_load_text(cpu_load: ProcessTreeCpuLoad) -> Option<String> {
    match cpu_load {
        ProcessTreeCpuLoad::AwaitingSecondSample => None,
        ProcessTreeCpuLoad::BusyCores(cores) => Some(format!("{cores:.1} cpu")),
    }
}

#[must_use]
pub(crate) fn approximate_memory_text(resident_memory: ByteSize) -> String {
    format!("~{resident_memory}")
}
