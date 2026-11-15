mod agenda_menu;
mod appearance_menu;
mod application_bar_menu;
mod applications_menu;
mod behaviour_menu;
mod display_menu;
mod hardware_menu;
mod media_menu;
mod network_menu;
mod services_menu;
mod session_menu;
mod virtualization_menu;

use sottomano_widgets::{
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
    widget_groups::{
        widget_group_content::WidgetGroupContent, widget_group_interaction::WidgetGroupInteraction,
    },
};

use crate::{desktop_shell::DesktopShell, shell_message::ShellInteraction};

impl DesktopShell {
    pub(crate) fn group_menu_of(
        &self,
        topic: GroupMenuTopic,
        output: &str,
    ) -> GroupMenuView<'_, ShellInteraction> {
        match topic {
            GroupMenuTopic::Applications => self.applications_group_menu(output),
            GroupMenuTopic::ApplicationMenu => self.application_bar_group_menu(),
            GroupMenuTopic::Hardware => self.hardware_group_menu(),
            GroupMenuTopic::Network => self.network_group_menu(),
            GroupMenuTopic::Agenda => self.agenda_group_menu(),
            GroupMenuTopic::Media => self.media_group_menu(),
            GroupMenuTopic::Display => self.display_group_menu(),
            GroupMenuTopic::Virtualization => self.virtualization_group_menu(),
            GroupMenuTopic::Services => self.services_group_menu(),
            GroupMenuTopic::Session => self.session_group_menu(),
            GroupMenuTopic::Appearance => self.appearance_group_menu(),
            GroupMenuTopic::Behaviour => self.behaviour_group_menu(),
        }
    }

    fn group_menu<'a>(
        &'a self,
        topic: GroupMenuTopic,
        groups: impl IntoIterator<Item = WidgetGroupContent<'a, WidgetGroupInteraction>>,
    ) -> GroupMenuView<'a, ShellInteraction> {
        GroupMenuView::new(
            topic,
            &self.widget_groups.group_menus,
            |menu_message| {
                ShellInteraction::Group(WidgetGroupInteraction::GroupMenus(menu_message))
            },
            groups
                .into_iter()
                .map(|group| group.map(ShellInteraction::Group)),
        )
    }

    fn settings_groups(&self) -> [WidgetGroupContent<'_, WidgetGroupInteraction>; 6] {
        let config = &self.config;
        [
            self.widget_groups
                .panel_settings_group
                .group_content(config)
                .map(WidgetGroupInteraction::PanelSettings),
            self.widget_groups
                .sidebar_settings_group
                .group_content(config)
                .map(WidgetGroupInteraction::SidebarSettings),
            self.widget_groups
                .overlay_settings_group
                .group_content(config)
                .map(WidgetGroupInteraction::OverlaySettings),
            self.widget_groups
                .compositor_styling_group
                .group_content(config)
                .map(WidgetGroupInteraction::CompositorStyling),
            self.widget_groups
                .group_menus
                .settings_content(config)
                .map(WidgetGroupInteraction::GroupMenus),
            self.widget_groups
                .global_menu_group
                .switch_content()
                .map(WidgetGroupInteraction::GlobalMenu),
        ]
    }
}
