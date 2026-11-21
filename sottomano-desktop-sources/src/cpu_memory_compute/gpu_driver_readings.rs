use bytesize::ByteSize;
use uom::si::f64::{Frequency, Power, Ratio, ThermodynamicTemperature};

use super::memory_usage::Memory;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuClockSpeed {
    NotExposed,
    Reported(Frequency),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuPowerLimit {
    NotExposed,
    Enforced(Power),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuPowerReading {
    NotExposed,
    Drawing(Power),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuTemperatureThreshold {
    NotExposed,
    Reported(ThermodynamicTemperature),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuTemperatureReading {
    NoSensor,
    Measured(ThermodynamicTemperature),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPerformanceState {
    NotExposed,
    PowerState(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuFanDuty {
    NotExposed,
    Spinning(Ratio),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuProcessName {
    Unreadable,
    Command(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuProcessMemory {
    NotReported,
    Resident(ByteSize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SystemMemoryReading {
    Unreadable,
    Read(Memory),
}
