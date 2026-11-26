use sottomano_widgets::{
    group_indicators::indicator_group_content::IndicatorGroupContent,
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn appearance_group_menu(&self) -> GroupMenuView<'_> {
        self.group_menu(
            GroupMenuTopic::Appearance,
            self.settings_groups()
                .into_iter()
                .filter_map(IndicatorGroupContent::only_aligned_indicators),
        )
    }
}
