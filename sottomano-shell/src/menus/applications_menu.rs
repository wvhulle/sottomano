use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn applications_group_menu(
        &self,
        output: &str,
    ) -> GroupMenuView<'_, ShellInteraction> {
        self.group_menu(
            GroupMenuTopic::Applications,
            self.widget_groups
                .app_launcher_group
                .group_contents(&self.desktop_entries, &self.config, output)
                .into_iter()
                .map(|group| group.map(ShellInteraction::AppLauncher)),
        )
    }
}
