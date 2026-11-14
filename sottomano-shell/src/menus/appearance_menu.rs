use sottomano_widgets::{
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
    widget_groups::widget_group_content::WidgetGroupContent,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn appearance_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        self.group_menu(
            GroupMenuTopic::Appearance,
            self.settings_groups()
                .map(WidgetGroupContent::only_aligned_items),
        )
    }
}
