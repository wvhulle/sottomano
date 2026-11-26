use sottomano_widgets::{
    group_menus::{group_menu_state::GroupMenuTopic, group_menu_view::GroupMenuView},
    panel_menus::agenda_menu::notification_group::NotificationStatus,
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(crate) fn agenda_group_menu(&self) -> GroupMenuView<'_> {
        let config = &self.config;
        self.group_menu(
            GroupMenuTopic::Agenda,
            [
                self.indicator_groups
                    .clock_group
                    .group_content(self.clock, config.calendar.locale),
                self.indicator_groups.caldav_group.group_content(
                    &config.indicator_groups.caldav,
                    self.clock,
                    config.calendar.locale,
                ),
                self.indicator_groups
                    .weather_group
                    .group_content(config, self.clock),
                self.indicator_groups.location_group.group_content(),
                self.indicator_groups
                    .quiet_schedule_group
                    .group_content(config),
                self.indicator_groups.notification_group.group_content(
                    self.notification_status(),
                    &self.indicator_groups.quiet_schedule_group,
                ),
                self.indicator_groups
                    .session_focus_group
                    .group_content(config, self.clock),
            ],
        )
    }

    pub(crate) fn notification_status(&self) -> NotificationStatus {
        NotificationStatus {
            quiet_phase: self.indicator_groups.quiet_schedule_group.phase,
            live: self.notifications.live.len(),
            held: self.notifications.held.len(),
        }
    }
}
