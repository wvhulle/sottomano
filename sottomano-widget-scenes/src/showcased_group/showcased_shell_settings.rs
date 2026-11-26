use sottomano_toml_config::Config;
use sottomano_widgets::{
    group_indicators::indicator_group_content::IndicatorGroupContent,
    group_menus::group_menu_state::GroupMenuState,
    panel_menus::settings_menu::{
        compositor_styling_group::CompositorStylingGroup,
        overlay_settings_group::OverlaySettingsGroup, panel_settings_group::PanelSettingsGroup,
        sidebar_settings_group::SidebarSettingsGroup,
    },
};

pub struct ShowcasedShellSettings {
    pub panels: PanelSettingsGroup,
    pub sidebar: SidebarSettingsGroup,
    pub overlays: OverlaySettingsGroup,
    pub menus: GroupMenuState,
    pub compositor_styling: CompositorStylingGroup,
}

impl ShowcasedShellSettings {
    pub(super) fn group_contents<'view>(
        &'view self,
        config: &'view Config,
    ) -> Vec<IndicatorGroupContent<'view>> {
        vec![
            self.panels.group_content(config),
            self.sidebar.group_content(config),
            self.overlays.group_content(config),
            self.compositor_styling.group_content(config),
            self.menus.settings_content(config),
        ]
    }
}
