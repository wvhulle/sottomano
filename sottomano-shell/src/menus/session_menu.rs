use sottomano_desktop_sources::desktop_windows_apps::window_edge_placement::Placement;
use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn session_group_menu(&self) -> GroupMenuView<'_> {
        let window_placement = Placement::derive(&self.compositor);
        self.group_menu(
            GroupMenuTopic::Session,
            [
                self.indicator_groups
                    .idle_pause_groups
                    .idle_chain_group(&self.config, self.clock),
                self.indicator_groups.login_session_group.group_content(),
                self.indicator_groups.tray_group.group_content(
                    &self.config,
                    &self.icon_catalog,
                    &|owner| window_placement.window_owned_by(owner).is_some(),
                ),
            ],
        )
    }
}
