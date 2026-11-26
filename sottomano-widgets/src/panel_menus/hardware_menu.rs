mod block_filesystem_discovery_indicator;
pub mod compute_group;
pub mod gpu_group;
pub mod memory_board_group;
pub mod physical_drive_group;
pub mod power_group;
pub mod storage_space_group;

use sottomano_toml_config::Config;

use self::{
    compute_group::ComputeGroup, gpu_group::GpuGroup, memory_board_group::MemoryBoardGroup,
    physical_drive_group::PhysicalDriveGroup, power_group::PowerGroup,
    storage_space_group::StorageSpaceGroup,
};
use crate::{clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent};

#[derive(Default)]
pub struct HardwareMenu {
    pub power_group: PowerGroup,
    pub compute_group: ComputeGroup,
    pub memory_board_group: MemoryBoardGroup,
    pub gpu_group: GpuGroup,
    pub storage_space_group: StorageSpaceGroup,
    pub physical_drive_group: PhysicalDriveGroup,
}

impl HardwareMenu {
    pub fn collapse(&mut self) {
        self.power_group.collapse();
        self.compute_group.collapse();
        self.memory_board_group.collapse();
        self.gpu_group.collapse();
        self.storage_space_group.collapse();
        self.physical_drive_group.collapse();
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        let locale = config.calendar.locale;
        [
            self.power_group.group_content(config),
            self.compute_group.group_content(clock, locale),
            self.memory_board_group.group_content(locale),
        ]
        .into_iter()
        .chain(self.gpu_group.group_contents())
        .chain(self.storage_space_group.group_contents(locale))
        .chain(self.physical_drive_group.group_contents(locale))
        .collect()
    }
}
