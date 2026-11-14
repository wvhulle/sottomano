use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn application_bar_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        self.group_menu(
            GroupMenuTopic::ApplicationMenu,
            self.widget_groups
                .global_menu_group
                .group_contents()
                .into_iter()
                .map(|group| group.map(ShellInteraction::GlobalMenu)),
        )
    }
}
