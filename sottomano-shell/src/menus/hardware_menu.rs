use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn hardware_group_menu(&self) -> GroupMenuView<'_> {
        let config = &self.config;
        let groups = [
            self.indicator_groups.power_group.group_content(config),
            self.indicator_groups
                .compute_group
                .group_content(self.clock, self.config.calendar.locale),
            self.indicator_groups
                .memory_board_group
                .group_content(self.config.calendar.locale),
        ]
        .into_iter()
        .chain(self.indicator_groups.gpu_group.group_contents())
        .chain(
            self.indicator_groups
                .storage_space_group
                .group_contents(self.config.calendar.locale),
        )
        .chain(
            self.indicator_groups
                .physical_drive_group
                .group_contents(self.config.calendar.locale),
        );
        self.group_menu(GroupMenuTopic::Hardware, groups)
    }
}
