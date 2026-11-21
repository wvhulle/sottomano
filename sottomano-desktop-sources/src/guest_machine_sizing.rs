use bytesize::ByteSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestMemorySize {
    Configured(ByteSize),
    HypervisorDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestCoreCount {
    Configured(u32),
    HypervisorDefault,
}
