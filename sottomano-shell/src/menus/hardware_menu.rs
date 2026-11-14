use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn hardware_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        let groups = [
            self.widget_groups
                .power_group
                .group_content(config)
                .map(ShellInteraction::Power),
            self.widget_groups
                .compute_group
                .group_content()
                .map(ShellInteraction::Compute),
            self.widget_groups
                .memory_board_group
                .group_content()
                .map(ShellInteraction::MemoryBoard),
        ]
        .into_iter()
        .chain(
            self.widget_groups
                .gpu_group
                .group_contents()
                .map(|group| group.map(ShellInteraction::Gpu)),
        )
        .chain(
            self.widget_groups
                .storage_space_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(ShellInteraction::Storage)),
        )
        .chain(
            self.widget_groups
                .physical_drive_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(ShellInteraction::Drive)),
        );
        self.group_menu(GroupMenuTopic::Hardware, groups)
    }
}
