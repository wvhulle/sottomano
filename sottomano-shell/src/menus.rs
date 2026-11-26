use sottomano_desktop_sources::desktop_windows_apps::window_edge_placement::Placement;
use sottomano_widgets::{
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
    panel_menus::agenda_menu::notification_group::pending_notifications_indicator::NotificationStatus,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn topic_group_menu(&self, topic: GroupMenuTopic) -> GroupMenuView<'_> {
        let config = &self.config;
        let groups = &self.indicator_groups;
        let menus = &groups.panel_menus;
        let group_menus = &groups.group_menus;
        let group_contents = match topic {
            GroupMenuTopic::Applications
            | GroupMenuTopic::WindowMenuBar
            | GroupMenuTopic::SessionPower => Vec::new(),
            GroupMenuTopic::Hardware => menus.hardware.group_contents(config, self.clock),
            GroupMenuTopic::Network => menus
                .network
                .group_contents(&menus.gaming.gamepad_group, self.clock),
            GroupMenuTopic::Agenda => {
                menus
                    .agenda
                    .group_contents(config, self.notification_status(), self.clock)
            }
            GroupMenuTopic::Media => {
                menus
                    .media
                    .group_contents(config, &self.desktop_entries, self.clock)
            }
            GroupMenuTopic::Gaming => {
                return GroupMenuView::new(
                    topic,
                    group_menus,
                    menus
                        .gaming
                        .group_contents(config, &self.icon_catalog, self.clock),
                )
                .with_handle_badge(
                    menus
                        .gaming
                        .steam_client_groups
                        .handle_badge(&self.icon_catalog),
                );
            }
            GroupMenuTopic::Display => {
                menus
                    .display
                    .group_contents(config, &self.outputs, self.clock)
            }
            GroupMenuTopic::Virtualization => menus.virtualization.group_contents(config),
            GroupMenuTopic::Services => menus.services.group_contents(config, self.clock),
            GroupMenuTopic::Session => {
                let window_placement = Placement::derive(&self.compositor);
                menus.session.group_contents(
                    config,
                    &self.icon_catalog,
                    &|owner| window_placement.window_owned_by(owner).is_some(),
                    self.clock,
                )
            }
            GroupMenuTopic::Appearance => {
                menus
                    .appearance
                    .group_contents(config, group_menus, &groups.window_menu_bar)
            }
            GroupMenuTopic::Behaviour => menus.behaviour.group_contents(
                &menus.appearance,
                config,
                group_menus,
                &groups.window_menu_bar,
                &groups.shell_appearance(),
            ),
        };
        GroupMenuView::new(topic, group_menus, group_contents)
    }

    pub(crate) fn notification_status(&self) -> NotificationStatus {
        NotificationStatus {
            quiet_phase: self
                .indicator_groups
                .panel_menus
                .agenda
                .quiet_schedule_group
                .phase,
            live: self.notifications.live.len(),
            held: self.notifications.held.len(),
        }
    }
}
