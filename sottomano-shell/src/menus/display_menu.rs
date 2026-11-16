use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn display_group_menu(&self) -> GroupMenuView<'_> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Display,
            [
                self.widget_groups
                    .monitor_group
                    .group_content(config, &self.outputs, self.clock),
                self.widget_groups
                    .dark_mode_group
                    .group_content(config, self.clock),
                self.widget_groups
                    .color_temperature_group
                    .group_content(config),
                self.widget_groups.wallpaper_group.group_content(self.clock),
            ],
        )
    }
}
