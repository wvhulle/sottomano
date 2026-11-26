pub mod compositor_styling_group;
pub mod overlay_settings_group;
pub mod panel_settings_group;
pub mod sidebar_settings_group;

use sottomano_desktop_sinks::compositor_rule_files::CompositorRulesFile;
use sottomano_toml_config::Config;

use self::{
    compositor_styling_group::CompositorStylingGroup, overlay_settings_group::OverlaySettingsGroup,
    panel_settings_group::PanelSettingsGroup, sidebar_settings_group::SidebarSettingsGroup,
};
use crate::{
    cascading_menus::window_menu_bar::WindowMenuBar,
    group_indicators::indicator_group_content::IndicatorGroupContent,
    group_menus::group_menu_state::GroupMenuState,
};

pub struct AppearanceMenu {
    pub panel_settings_group: PanelSettingsGroup,
    pub sidebar_settings_group: SidebarSettingsGroup,
    pub overlay_settings_group: OverlaySettingsGroup,
    pub compositor_styling_group: CompositorStylingGroup,
}

impl AppearanceMenu {
    pub fn collapse(&mut self) {
        self.panel_settings_group.collapse();
        self.sidebar_settings_group.collapse();
        self.overlay_settings_group.collapse();
        self.compositor_styling_group.collapse();
    }

    #[must_use]
    pub fn new(compositor_rules: CompositorRulesFile) -> Self {
        Self {
            panel_settings_group: PanelSettingsGroup::default(),
            sidebar_settings_group: SidebarSettingsGroup::default(),
            overlay_settings_group: OverlaySettingsGroup::default(),
            compositor_styling_group: CompositorStylingGroup::new(compositor_rules),
        }
    }

    #[must_use]
    pub fn group_contents<'view>(
        &'view self,
        config: &'view Config,
        group_menus: &'view GroupMenuState,
        window_menu_bar: &'view WindowMenuBar,
    ) -> Vec<IndicatorGroupContent<'view>> {
        self.settings_groups(config, group_menus, window_menu_bar)
            .into_iter()
            .filter_map(IndicatorGroupContent::only_aligned_indicators)
            .collect()
    }

    pub fn behaviour_settings_groups<'view>(
        &'view self,
        config: &'view Config,
        group_menus: &'view GroupMenuState,
        window_menu_bar: &'view WindowMenuBar,
    ) -> impl Iterator<Item = IndicatorGroupContent<'view>> {
        self.settings_groups(config, group_menus, window_menu_bar)
            .into_iter()
            .filter_map(IndicatorGroupContent::only_unaligned_indicators)
    }

    fn settings_groups<'view>(
        &'view self,
        config: &'view Config,
        group_menus: &'view GroupMenuState,
        window_menu_bar: &'view WindowMenuBar,
    ) -> [IndicatorGroupContent<'view>; 6] {
        [
            self.panel_settings_group.group_content(config),
            self.sidebar_settings_group.group_content(config),
            self.overlay_settings_group.group_content(config),
            self.compositor_styling_group.group_content(config),
            group_menus.settings_content(config),
            window_menu_bar.switch_content(),
        ]
    }
}
