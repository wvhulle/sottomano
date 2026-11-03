use std::path::Path;

use sottomano_widgets::{
    clock::Clock,
    group_menus::group_menu_state::GroupMenuState,
    widget_groups::{
        niri_window_group::{NiriWindowGroup, niri_kdl_rules::NiriRulesFile},
        overlay_settings_group::OverlaySettingsGroup,
        panel_settings_group::PanelSettingsGroup,
        sidebar_settings_group::SidebarSettingsGroup,
    },
};

use crate::showcased_group::{ShowcasedGroup, ShowcasedShellSettings, ShownSettingItems};

const UNREAD_NIRI_CONFIG_HOME: &str = "showcase-never-reads-this";

fn settings_showing(shown_items: ShownSettingItems) -> ShowcasedGroup {
    ShowcasedGroup::ShellSettings(Box::new(ShowcasedShellSettings {
        panels: PanelSettingsGroup::default(),
        sidebar: SidebarSettingsGroup::default(),
        overlays: OverlaySettingsGroup::default(),
        menus: GroupMenuState::default(),
        niri_window: NiriWindowGroup::new(NiriRulesFile::in_config_home(Path::new(
            UNREAD_NIRI_CONFIG_HOME,
        ))),
        shown_items,
    }))
}

pub fn sharing_a_unit(_clock: Clock) -> ShowcasedGroup {
    settings_showing(ShownSettingItems::SharingAUnit)
}
