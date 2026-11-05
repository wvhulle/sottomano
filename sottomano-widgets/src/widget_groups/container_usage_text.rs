use bytesize::ByteSize;
use sottomano_desktop_sources::podman_container_engine::libpod_running_containers::ContainerCpuLoad;

#[must_use]
pub(crate) fn cpu_load_text(cpu_load: ContainerCpuLoad) -> Option<String> {
    match cpu_load {
        ContainerCpuLoad::AwaitingSecondSample => None,
        ContainerCpuLoad::BusyCores(cores) => Some(format!("{cores:.1} cpu")),
    }
}

#[must_use]
pub(crate) fn approximate_memory_text(resident_memory: ByteSize) -> String {
    format!("~{resident_memory}")
}
