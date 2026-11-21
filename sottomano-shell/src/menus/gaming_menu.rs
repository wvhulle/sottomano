use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn gaming_group_menu(&self) -> GroupMenuView<'_> {
        let steam_client_groups = &self.indicator_groups.steam_client_groups;
        let controller_groups = self
            .indicator_groups
            .gamepad_group
            .group_contents(&self.config.indicator_groups.gamepad);
        let steam_groups = steam_client_groups.group_contents(
            self.clock,
            self.config.calendar.locale,
            &self.icon_catalog,
        );
        self.group_menu(
            GroupMenuTopic::Gaming,
            controller_groups.into_iter().chain(steam_groups),
        )
        .with_handle_badge(steam_client_groups.handle_badge(&self.icon_catalog))
    }
}
