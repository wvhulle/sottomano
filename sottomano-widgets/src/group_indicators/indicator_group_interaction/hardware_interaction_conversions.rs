use super::IndicatorGroupInteraction;
use crate::panel_menus::{
    gaming_menu::gamepad_group::GamepadInteraction,
    hardware_menu::{
        compute_group::ComputeInteraction, gpu_group::GpuInteraction,
        memory_board_group::MemoryBoardInteraction, physical_drive_group::DriveInteraction,
        power_group::PowerInteraction, storage_space_group::StorageInteraction,
    },
};

impl From<PowerInteraction> for IndicatorGroupInteraction {
    fn from(interaction: PowerInteraction) -> Self {
        IndicatorGroupInteraction::Power(interaction)
    }
}

impl From<ComputeInteraction> for IndicatorGroupInteraction {
    fn from(interaction: ComputeInteraction) -> Self {
        IndicatorGroupInteraction::Compute(interaction)
    }
}

impl From<MemoryBoardInteraction> for IndicatorGroupInteraction {
    fn from(interaction: MemoryBoardInteraction) -> Self {
        IndicatorGroupInteraction::MemoryBoard(interaction)
    }
}

impl From<GamepadInteraction> for IndicatorGroupInteraction {
    fn from(interaction: GamepadInteraction) -> Self {
        IndicatorGroupInteraction::Gamepad(interaction)
    }
}

impl From<GpuInteraction> for IndicatorGroupInteraction {
    fn from(interaction: GpuInteraction) -> Self {
        IndicatorGroupInteraction::Gpu(interaction)
    }
}

impl From<StorageInteraction> for IndicatorGroupInteraction {
    fn from(interaction: StorageInteraction) -> Self {
        IndicatorGroupInteraction::Storage(interaction)
    }
}

impl From<DriveInteraction> for IndicatorGroupInteraction {
    fn from(interaction: DriveInteraction) -> Self {
        IndicatorGroupInteraction::Drive(interaction)
    }
}
