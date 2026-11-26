pub mod memory_growth_trend;
pub mod temperature_sensors;

use serde::{Deserialize, Serialize};

use self::{memory_growth_trend::MemoryGrowthTrendConfig, temperature_sensors::SensorConfig};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HardwareMenuConfig {
    pub compute: ComputeGroupConfig,
    pub memory_board: MemoryBoardGroupConfig,
    pub physical_drive: PhysicalDriveGroupConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ComputeGroupConfig {
    pub temperature_sensors: SensorConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MemoryBoardGroupConfig {
    pub growth_trend: MemoryGrowthTrendConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PhysicalDriveGroupConfig {
    pub temperature_sensors: SensorConfig,
}
