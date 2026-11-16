use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn hardware_group_menu(&self) -> GroupMenuView<'_> {
        let config = &self.config;
        let groups = [
            self.widget_groups
                .power_group
                .group_content(config, self.clock),
            self.widget_groups
                .compute_group
                .group_content(self.clock, self.config.calendar.locale),
            self.widget_groups
                .memory_board_group
                .group_content(self.clock, self.config.calendar.locale),
        ]
        .into_iter()
        .chain(self.widget_groups.gpu_group.group_contents(self.clock))
        .chain(
            self.widget_groups
                .storage_space_group
                .group_contents(self.clock, self.config.calendar.locale),
        )
        .chain(
            self.widget_groups
                .physical_drive_group
                .group_contents(self.clock, self.config.calendar.locale),
        );
        self.group_menu(GroupMenuTopic::Hardware, groups)
    }
}
