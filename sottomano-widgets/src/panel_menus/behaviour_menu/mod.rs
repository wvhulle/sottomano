pub mod appearance_presets_group;

use std::iter::once;

use sottomano_toml_config::Config;

use self::appearance_presets_group::AppearancePresetsGroup;
use crate::{
    cascading_menus::window_menu_bar::WindowMenuBar,
    group_indicators::indicator_group_content::IndicatorGroupContent,
    group_menus::group_menu_state::GroupMenuState, panel_menus::appearance_menu::AppearanceMenu,
    widgets::appearance_presets::ShellAppearance,
};

#[derive(Default)]
pub struct BehaviourMenu {
    pub appearance_presets_group: AppearancePresetsGroup,
}

impl BehaviourMenu {
    pub fn collapse(&mut self) {
        self.appearance_presets_group.collapse();
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        appearance_menu: &'view AppearanceMenu,
        config: &'view Config,
        group_menus: &'view GroupMenuState,
        window_menu_bar: &'view WindowMenuBar,
        shell_appearance: &ShellAppearance,
    ) -> Vec<IndicatorGroupContent<'view>> {
        appearance_menu
            .behaviour_settings_groups(config, group_menus, window_menu_bar)
            .chain(once(
                self.appearance_presets_group
                    .group_content(shell_appearance),
            ))
            .collect()
    }
}
