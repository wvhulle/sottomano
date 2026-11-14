use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn agenda_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Agenda,
            [
                self.widget_groups
                    .caldav_group
                    .group_content(&config.widget_groups.caldav)
                    .map(ShellInteraction::Caldav),
                self.widget_groups
                    .weather_group
                    .group_content(config)
                    .map(ShellInteraction::Weather),
                self.widget_groups
                    .quiet_schedule_group
                    .group_content(config)
                    .map(ShellInteraction::QuietSchedule),
                self.widget_groups
                    .session_focus_group
                    .group_content(config)
                    .map(ShellInteraction::SessionFocus),
            ],
        )
    }
}
