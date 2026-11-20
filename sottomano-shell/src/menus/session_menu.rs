use std::iter::once;

use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn session_group_menu(&self) -> GroupMenuView<'_> {
        let idle_chain = self
            .indicator_groups
            .idle_pause_groups
            .idle_chain_group(&self.config, self.clock);
        let login_session = self.indicator_groups.login_session_group.group_content();
        self.group_menu(
            GroupMenuTopic::Session,
            once(idle_chain).chain(once(login_session)),
        )
    }
}
