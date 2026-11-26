use std::path::Path;

use sottomano_compositors::compositor_connection::CompositorKind;
use sottomano_desktop_sinks::compositor_rule_files::CompositorRulesFile;
use sottomano_widgets::{
    group_menus::group_menu_state::GroupMenuState,
    panel_menus::settings_menu::{
        compositor_styling_group::CompositorStylingGroup,
        overlay_settings_group::OverlaySettingsGroup, panel_settings_group::PanelSettingsGroup,
        sidebar_settings_group::SidebarSettingsGroup,
    },
};

use crate::showcased_group::{ShowcasedGroup, showcased_shell_settings::ShowcasedShellSettings};

const UNREAD_CONFIG_HOME: &str = "showcase-never-reads-this";

pub fn surface_settings() -> ShowcasedGroup {
    ShowcasedGroup::ShellSettings(Box::new(ShowcasedShellSettings {
        panels: PanelSettingsGroup::default(),
        sidebar: SidebarSettingsGroup::default(),
        overlays: OverlaySettingsGroup::default(),
        menus: GroupMenuState::default(),
        compositor_styling: CompositorStylingGroup::new(CompositorRulesFile::in_config_home(
            Path::new(UNREAD_CONFIG_HOME),
            CompositorKind::COMPILED_IN.first().copied(),
        )),
    }))
}
