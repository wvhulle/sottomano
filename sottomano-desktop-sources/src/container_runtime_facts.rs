use std::time::SystemTime;

use bytesize::ByteSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerMemoryUse {
    Measured(ByteSize),
    Unaccounted,
}

impl From<Option<ByteSize>> for ContainerMemoryUse {
    fn from(cgroup_memory: Option<ByteSize>) -> Self {
        cgroup_memory.map_or(
            ContainerMemoryUse::Unaccounted,
            ContainerMemoryUse::Measured,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerStartTime {
    StartedAt(SystemTime),
    Unrecorded,
}

impl From<Option<SystemTime>> for ContainerStartTime {
    fn from(recorded_start: Option<SystemTime>) -> Self {
        recorded_start.map_or(
            ContainerStartTime::Unrecorded,
            ContainerStartTime::StartedAt,
        )
    }
}
