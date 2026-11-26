pub mod gamepad_group;
pub mod steam_client_groups;

use sottomano_toml_config::Config;

use self::{gamepad_group::GamepadGroup, steam_client_groups::SteamClientGroups};
use crate::{
    clock::Clock, group_indicators::indicator_group_content::IndicatorGroupContent,
    icon_catalog::drawable_icon_catalog::IconCatalog,
};

#[derive(Default)]
pub struct GamingMenu {
    pub gamepad_group: GamepadGroup,
    pub steam_client_groups: SteamClientGroups,
}

impl GamingMenu {
    pub fn collapse(&mut self) {
        self.gamepad_group.collapse();
        self.steam_client_groups.collapse();
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        icon_catalog: &'view IconCatalog,
        clock: Clock,
    ) -> Vec<IndicatorGroupContent<'view>> {
        self.gamepad_group
            .group_contents(&config.panel_menus.gaming.gamepad, clock)
            .into_iter()
            .chain(self.steam_client_groups.group_contents(
                clock,
                config.calendar.locale,
                icon_catalog,
            ))
            .collect()
    }
}
