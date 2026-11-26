pub mod color_temperature_group;
pub mod dark_mode_group;
pub mod monitor_group;
pub mod wallpaper_group;

use std::collections::HashMap;

use sottomano_compositor_model::compositor_output_facts::OutputInfo;
use sottomano_toml_config::{Config, SecretPaths};

use self::{
    color_temperature_group::ColorTemperatureGroup, dark_mode_group::DarkModeGroup,
    monitor_group::MonitorGroup, wallpaper_group::WallpaperGroup,
};
use crate::{clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent};

pub struct DisplayMenu {
    pub monitor_group: MonitorGroup,
    pub dark_mode_group: DarkModeGroup,
    pub color_temperature_group: ColorTemperatureGroup,
    pub wallpaper_group: WallpaperGroup,
}

impl DisplayMenu {
    pub fn collapse(&mut self) {
        self.monitor_group.collapse();
        self.dark_mode_group.collapse();
        self.color_temperature_group.collapse();
        self.wallpaper_group.collapse();
    }

    #[must_use]
    pub fn new(config: &Config, secrets: &SecretPaths, clock: Clock) -> Self {
        Self {
            monitor_group: MonitorGroup::default(),
            dark_mode_group: DarkModeGroup::new(clock),
            color_temperature_group: ColorTemperatureGroup::new(
                config.launched_programs.color_temperature_service.clone(),
            ),
            wallpaper_group: WallpaperGroup::new(secrets),
        }
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        outputs: &'view HashMap<String, OutputInfo>,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        vec![
            self.monitor_group.group_content(config, outputs),
            self.dark_mode_group.group_content(config, clock),
            self.color_temperature_group.group_content(config),
            self.wallpaper_group.group_content(),
        ]
    }
}
