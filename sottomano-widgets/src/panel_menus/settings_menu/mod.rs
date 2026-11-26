pub mod appearance_presets_group;
pub mod compositor_styling_group;
pub mod launcher_settings_group;
pub mod overlay_settings_group;
pub mod panel_settings_group;
pub mod sidebar_settings_group;

use sottomano_desktop_sinks::compositor_rule_files::CompositorRulesFile;
use sottomano_toml_config::Config;

use self::{
    appearance_presets_group::AppearancePresetsGroup,
    compositor_styling_group::CompositorStylingGroup,
    launcher_settings_group::launcher_settings_group, overlay_settings_group::OverlaySettingsGroup,
    panel_settings_group::PanelSettingsGroup, sidebar_settings_group::SidebarSettingsGroup,
};
use crate::{
    cascading_menus::{app_launcher_menu::AppLauncherMenu, window_menu_bar::WindowMenuBar},
    group_indicators::indicator_group_content::IndicatorGroupContent,
    group_menus::group_menu_state::GroupMenuState,
    widgets::appearance_presets::ShellAppearance,
};

pub struct SettingsMenu {
    pub appearance_presets_group: AppearancePresetsGroup,
    pub panel_settings_group: PanelSettingsGroup,
    pub sidebar_settings_group: SidebarSettingsGroup,
    pub overlay_settings_group: OverlaySettingsGroup,
    pub compositor_styling_group: CompositorStylingGroup,
}

impl SettingsMenu {
    pub fn collapse(&mut self) {
        self.appearance_presets_group.collapse();
        self.panel_settings_group.collapse();
        self.sidebar_settings_group.collapse();
        self.overlay_settings_group.collapse();
        self.compositor_styling_group.collapse();
    }

    #[must_use]
    pub fn new(compositor_rules: CompositorRulesFile) -> Self {
        Self {
            appearance_presets_group: AppearancePresetsGroup::default(),
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
        app_launcher_menu: &'view AppLauncherMenu,
        shell_appearance: &ShellAppearance,
    ) -> Vec<IndicatorGroupContent<'view>> {
        vec![
            self.appearance_presets_group
                .group_content(shell_appearance),
            self.panel_settings_group.group_content(config),
            self.sidebar_settings_group.group_content(config),
            self.overlay_settings_group.group_content(config),
            self.compositor_styling_group.group_content(config),
            group_menus.settings_content(config),
            window_menu_bar.switch_content(),
            launcher_settings_group(app_launcher_menu),
        ]
    }
}
