use sottomano_toml_config::Config;
use sottomano_widgets::{
    group_menus::group_menu_state::GroupMenuState,
    indicator_groups::{
        compositor_styling_group::CompositorStylingGroup,
        indicator_group_content::IndicatorGroupContent,
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
    pub shown_items: ShownSettingItems,
}

#[derive(Clone, Copy)]
pub enum ShownSettingItems {
    SharingAUnit,
    Remaining,
}

impl ShowcasedShellSettings {
    pub(super) fn group_contents<'view>(
        &'view self,
        config: &'view Config,
    ) -> Vec<IndicatorGroupContent<'view>> {
        let inert_settings = [
            self.panels.group_content(config),
            self.sidebar.group_content(config),
            self.overlays.group_content(config),
            self.compositor_styling.group_content(config),
            self.menus.settings_content(config),
        ];
        match self.shown_items {
            ShownSettingItems::SharingAUnit => inert_settings
                .into_iter()
                .filter_map(IndicatorGroupContent::only_aligned_indicators)
                .collect(),
            ShownSettingItems::Remaining => inert_settings
                .into_iter()
                .filter_map(IndicatorGroupContent::only_unaligned_indicators)
                .collect(),
        }
    }
}
