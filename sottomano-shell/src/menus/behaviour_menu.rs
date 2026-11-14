use std::iter::once;

use sottomano_widgets::{
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
    widget_groups::widget_group_content::WidgetGroupContent,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn behaviour_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        self.group_menu(
            GroupMenuTopic::Behaviour,
            self.settings_groups()
                .map(WidgetGroupContent::only_unaligned_items)
                .into_iter()
                .chain(once(
                    self.widget_groups
                        .appearance_presets_group
                        .group_content(&self.widget_groups.shell_appearance())
                        .map(ShellInteraction::AppearancePresets),
                )),
        )
    }
}
