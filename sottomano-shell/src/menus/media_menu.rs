use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn media_group_menu(&self) -> GroupMenuView<'_, ShellInteraction> {
        let config = &self.config;
        let media_groups = self
            .widget_groups
            .media_group
            .group_contents(config, self.clock)
            .map(|group| group.map(ShellInteraction::Media));
        let gamepad_groups = self
            .widget_groups
            .gamepad_group
            .group_contents(&config.widget_groups.gamepad)
            .into_iter()
            .map(|group| group.map(ShellInteraction::Gamepad));
        let groups = media_groups
            .chain([
                self.widget_groups
                    .audio_group
                    .group_content(config)
                    .map(ShellInteraction::Audio),
                self.widget_groups
                    .voice_dictation_group
                    .group_content()
                    .map(ShellInteraction::VoiceDictation),
            ])
            .chain(gamepad_groups);
        self.group_menu(GroupMenuTopic::Media, groups)
    }
}
