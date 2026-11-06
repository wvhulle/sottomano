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

#[must_use]
pub(crate) fn usage_text(cpu_load: ProcessTreeCpuLoad, resident_memory: ByteSize) -> String {
    cpu_load_text(cpu_load)
        .into_iter()
        .chain([approximate_memory_text(resident_memory)])
        .collect::<Vec<String>>()
        .join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_is_marked_approximate_and_cpu_is_in_cores() {
        assert_eq!(
            usage_text(ProcessTreeCpuLoad::BusyCores(6.25), ByteSize::mib(640)),
            "6.2 cpu  ~640.0 MiB"
        );
    }

    #[test]
    fn a_tree_without_a_second_sample_shows_only_memory() {
        assert_eq!(
            usage_text(ProcessTreeCpuLoad::AwaitingSecondSample, ByteSize::mib(640)),
            "~640.0 MiB"
        );
    }
}
