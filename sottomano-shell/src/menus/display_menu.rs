use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn display_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Display,
            [
                self.widget_groups
                    .monitor_group
                    .group_content(config)
                    .map(ShellInteraction::Monitor),
                self.widget_groups
                    .dark_mode_group
                    .group_content(config)
                    .map(ShellInteraction::DarkMode),
                self.widget_groups
                    .color_temperature_group
                    .group_content(config)
                    .map(ShellInteraction::ColorTemperature),
                self.widget_groups
                    .wallpaper_group
                    .group_content()
                    .map(ShellInteraction::Wallpaper),
            ],
        )
    }
}
