use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn agenda_group_menu(&self) -> GroupMenuView<'_> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Agenda,
            [
                self.indicator_groups.caldav_group.group_content(
                    &config.indicator_groups.caldav,
                    self.clock,
                    self.config.calendar.locale,
                ),
                self.indicator_groups
                    .weather_group
                    .group_content(config, self.clock),
                self.indicator_groups
                    .quiet_schedule_group
                    .group_content(config),
                self.indicator_groups
                    .session_focus_group
                    .group_content(config, self.clock),
            ],
        )
    }
}
