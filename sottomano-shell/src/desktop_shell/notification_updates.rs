use std::collections::HashMap;

use iced::{Task, window::Id};
use sottomano_widgets::{
    notification_toasts::ToastContext,
    toast_messages::NotificationMessage,
    widget_groups::{
        notification_group::{self, NotificationGroupMessage},
        quiet_schedule_group::{QuietChange, QuietScheduleMessage, QuietUpdate},
    },
    widgets::widget_group::GroupToastRequests,
};

use crate::{
    desktop_shell::{DesktopShell, bottom_bar::BottomBarGroups, top_bar::TopBarGroups},
    shell_message::Message,
};

impl DesktopShell {
    pub(super) fn group_toast_requests(&self) -> GroupToastRequests<Message> {
        let locale = self.config.calendar.locale;
        let TopBarGroups { start, center, end } = self.top_bar_groups();
        let BottomBarGroups {
            start: bottom_start,
            end: bottom_end,
        } = self.bottom_bar_groups();
        start
            .iter()
            .chain(&center)
            .chain(&end)
            .chain(&bottom_start)
            .chain(&bottom_end)
            .map(|group| group.toast_requests(locale, self.clock))
            .collect()
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
                self.notification_settings(),
                self.quiet_schedule_group.is_quiet(),
                self.clock,
            ),
            &|output| toast_surfaces.get(output).copied(),
        )
    }

    pub fn update_notification_group(&mut self, msg: NotificationGroupMessage) -> Task<Message> {
        match self.notification_group.update(msg) {
            notification_group::Action::Run(task) => task.map(Message::NotificationGroup),
            notification_group::Action::ToggleDoNotDisturb => {
                self.update_quiet_schedule(QuietScheduleMessage::ToggleQuietNow)
            }
        }
    }

    pub fn update_quiet_schedule(&mut self, msg: QuietScheduleMessage) -> Task<Message> {
        let QuietUpdate { task, change } = self.quiet_schedule_group.update(msg, self.clock);
        Task::batch([
            task.map(Message::QuietSchedule),
            self.apply_quiet_change(change),
        ])
    }

    fn apply_quiet_change(&mut self, change: QuietChange) -> Task<Message> {
        match change {
            QuietChange::Unchanged => Task::none(),
            QuietChange::ScreenOffAfter(timeout) => self
                .idle_timeouts_group
                .override_screen_off(Some(timeout))
                .map(Message::IdleTimeouts),
            QuietChange::ScreenOffFollowsIdleTimer => self
                .idle_timeouts_group
                .override_screen_off(None)
                .map(Message::IdleTimeouts),
            QuietChange::Ended => {
                let released = self.notifications.release_held(&ToastContext::new(
                    &self.niri,
                    self.desktop_entries.get(),
                    &self.config,
                    self.notification_settings(),
                    false,
                    self.clock,
                ));
                Task::batch([
                    released,
                    self.idle_timeouts_group
                        .override_screen_off(None)
                        .map(Message::IdleTimeouts),
                ])
            }
        }
    }
}
