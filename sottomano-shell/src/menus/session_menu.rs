use std::iter::once;

use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn session_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let pause_groups = self
            .widget_groups
            .idle_pause_groups
            .pause_groups(&self.config)
            .into_iter()
            .map(|group| group.map(ShellInteraction::IdleTimeouts));
        let login_session = self
            .widget_groups
            .login_session_group
            .group_content()
            .map(ShellInteraction::LoginSession);
        self.group_menu(
            GroupMenuTopic::Session,
            pause_groups.chain(once(login_session)),
        )
    }
}
