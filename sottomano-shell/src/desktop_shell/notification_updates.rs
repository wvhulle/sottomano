use std::collections::HashMap;

use iced::{Task, window::Id};
use sottomano_widgets::{
    group_menus::group_menu_state::AlertPromotedGroup,
    notification_toasts::ToastContext,
    toast_messages::NotificationMessage,
    widget_groups::{
        notification_group,
        notification_group::NotificationGroupMessage,
        quiet_schedule_group::{QuietChange, QuietScheduleMessage, QuietUpdate},
        widget_group_content::GroupToastRequests,
    },
};

use crate::{
    desktop_shell::{
        DesktopShell,
        desktop_panels::panel_bar_composition::{composed_menu_topics, composed_standalone_groups},
    },
    shell_message::Message,
};

pub(super) struct GroupAlertScan {
    pub toast_requests: GroupToastRequests<Message>,
    pub promoted_groups: Vec<AlertPromotedGroup>,
}

impl DesktopShell {
    pub(super) fn scan_group_alerts(&self) -> GroupAlertScan {
        let locale = self.config.calendar.locale;
        let group_menus: Vec<_> = composed_menu_topics()
            .map(|topic| self.group_menu_of(topic))
            .collect();
        let toast_requests = composed_standalone_groups()
            .map(|group| {
                self.standalone_group(group)
                    .toast_requests(locale, self.clock)
            })
            .chain(
                group_menus
                    .iter()
                    .map(|group_menu| group_menu.toast_requests(locale, self.clock)),
            )
            .collect();
        let promotion_severity = self.config.group_menus.alert_promotion_severity;
        let promoted_groups = group_menus
            .iter()
            .flat_map(|group_menu| group_menu.promoted_groups(promotion_severity, self.clock))
            .collect();
        GroupAlertScan {
            toast_requests,
            promoted_groups,
        }
    }

    pub fn update_notification(&mut self, msg: NotificationMessage) -> Task<Message> {
        let toast_surfaces: HashMap<String, Id> = self
            .notification_toast_surfaces()
            .map(|(id, output)| (output.to_owned(), id))
            .collect();
        self.notifications.update(
            msg,
            &ToastContext::new(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                self.widget_groups.notification_settings(),
                self.widget_groups.quiet_schedule_group.is_quiet(),
                self.clock,
            ),
            &|output| toast_surfaces.get(output).copied(),
        )
    }

    pub fn update_notification_group(&mut self, msg: NotificationGroupMessage) -> Task<Message> {
        match self.widget_groups.notification_group.update(msg) {
            notification_group::Action::Run(task) => task.map(Message::NotificationGroup),
            notification_group::Action::ToggleDoNotDisturb => {
                self.update_quiet_schedule(QuietScheduleMessage::ToggleQuietNow)
            }
        }
    }

    pub fn update_quiet_schedule(&mut self, msg: QuietScheduleMessage) -> Task<Message> {
        let QuietUpdate { task, change } = self
            .widget_groups
            .quiet_schedule_group
            .update(msg, self.clock);
        Task::batch([
            task.map(Message::QuietSchedule),
            self.apply_quiet_change(change),
        ])
    }

    fn apply_quiet_change(&mut self, change: QuietChange) -> Task<Message> {
        match change {
            QuietChange::Unchanged => Task::none(),
            QuietChange::ScreenOffAfter(timeout) => self
                .widget_groups
                .idle_timeouts_group
                .override_screen_off(Some(timeout))
                .map(Message::IdleTimeouts),
            QuietChange::ScreenOffFollowsIdleTimer => self
                .widget_groups
                .idle_timeouts_group
                .override_screen_off(None)
                .map(Message::IdleTimeouts),
            QuietChange::Ended => {
                let released = self.notifications.release_held(&ToastContext::new(
                    &self.niri,
                    self.desktop_entries.get(),
                    &self.config,
                    self.widget_groups.notification_settings(),
                    false,
                    self.clock,
                ));
                Task::batch([
                    released,
                    self.widget_groups
                        .idle_timeouts_group
                        .override_screen_off(None)
                        .map(Message::IdleTimeouts),
                ])
            }
        }
    }
}
