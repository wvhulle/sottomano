use sottomano_widgets::group_menus::{
    group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn media_group_menu(&self) -> GroupMenuView<'_> {
        let config = &self.config;
        let media_groups = self.indicator_groups.media_group.group_contents(
            config,
            &self.desktop_entries,
            self.clock,
        );
        let gamepad_groups = self
            .indicator_groups
            .gamepad_group
            .group_contents(&config.indicator_groups.gamepad)
            .into_iter();
        let groups = media_groups
            .chain([
                self.indicator_groups.audio_group.group_content(config),
                self.indicator_groups.voice_dictation_group.group_content(),
            ])
            .chain(gamepad_groups);
        self.group_menu(GroupMenuTopic::Media, groups)
    }
}
