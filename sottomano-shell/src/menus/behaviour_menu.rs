use std::iter::once;

use sottomano_widgets::{
    group_indicators::indicator_group_content::IndicatorGroupContent,
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn behaviour_group_menu(&self) -> GroupMenuView<'_> {
        self.group_menu(
            GroupMenuTopic::Behaviour,
            self.settings_groups()
                .into_iter()
                .filter_map(IndicatorGroupContent::only_unaligned_indicators)
                .chain(once(
                    self.indicator_groups
                        .appearance_presets_group
                        .group_content(&self.indicator_groups.shell_appearance()),
                )),
        )
    }
}
