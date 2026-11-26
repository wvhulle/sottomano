use std::iter::empty;

mod agenda_menu;
mod appearance_menu;
mod behaviour_menu;
mod display_menu;
mod gaming_menu;
mod hardware_menu;
mod media_menu;
mod network_menu;
mod services_menu;
mod session_menu;
mod virtualization_menu;

use sottomano_widgets::{
    group_indicators::indicator_group_content::IndicatorGroupContent,
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn topic_group_menu(&self, topic: GroupMenuTopic) -> GroupMenuView<'_> {
        match topic {
            GroupMenuTopic::Applications
            | GroupMenuTopic::WindowMenuBar
            | GroupMenuTopic::SessionPower => self.group_menu(topic, empty()),
            GroupMenuTopic::Hardware => self.hardware_group_menu(),
            GroupMenuTopic::Network => self.network_group_menu(),
            GroupMenuTopic::Agenda => self.agenda_group_menu(),
            GroupMenuTopic::Media => self.media_group_menu(),
            GroupMenuTopic::Gaming => self.gaming_group_menu(),
            GroupMenuTopic::Display => self.display_group_menu(),
            GroupMenuTopic::Virtualization => self.virtualization_group_menu(),
            GroupMenuTopic::Services => self.services_group_menu(),
            GroupMenuTopic::Session => self.session_group_menu(),
            GroupMenuTopic::Appearance => self.appearance_group_menu(),
            GroupMenuTopic::Behaviour => self.behaviour_group_menu(),
        }
    }

    fn group_menu<'view>(
        &'view self,
        topic: GroupMenuTopic,
        groups: impl IntoIterator<Item = IndicatorGroupContent<'view>>,
    ) -> GroupMenuView<'view> {
        GroupMenuView::new(topic, &self.indicator_groups.group_menus, groups)
    }

    fn settings_groups(&self) -> [IndicatorGroupContent<'_>; 6] {
        let config = &self.config;
        [
            self.indicator_groups
                .panel_settings_group
                .group_content(config),
            self.indicator_groups
                .sidebar_settings_group
                .group_content(config),
            self.indicator_groups
                .overlay_settings_group
                .group_content(config),
            self.indicator_groups
                .compositor_styling_group
                .group_content(config),
            self.indicator_groups.group_menus.settings_content(config),
            self.indicator_groups.window_menu_bar.switch_content(),
        ]
    }
}
