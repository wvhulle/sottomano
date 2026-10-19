use std::collections::HashMap;

use iced::{Task, window::Id};
use sottomano_state::shell_settings;
use sottomano_widgets::{
    notification_toasts::ToastContext,
    shell_message::Message,
    toast_messages::NotificationMessage,
    widget_groups::{
        notification_group::NotificationGroupMessage,
        quiet_schedule_group::{QuietChange, QuietScheduleMessage, QuietUpdate},
    },
    widgets::widget_group::GroupToastRequests,
};

use crate::desktop_shell::{DesktopShell, bottom_bar::BottomBarGroups, top_bar::TopBarGroups};

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
        let panels: HashMap<String, Id> = self
            .panels()
            .map(|(id, output)| (output.to_owned(), id))
            .collect();
        self.notifications.update(
            msg,
            &ToastContext::new(
                &self.niri,
                self.desktop_entries.get(),
                &self.config,
                &self.shell_settings,
                self.quiet_schedule_group.is_quiet(),
                self.clock,
            ),
            &|output| panels.get(output).copied(),
        )
    }

    pub fn update_notification_group(&mut self, msg: NotificationGroupMessage) -> Task<Message> {
        match msg {
            NotificationGroupMessage::Expansion(menu) => {
                self.notification_group.update(menu);
                Task::none()
            }
            NotificationGroupMessage::ToggleSound => {
                let notifications = &mut self.shell_settings.notifications;
                notifications.sound = !notifications.sound;
                Task::future(shell_settings::save_sound(notifications.sound)).discard()
            }
            NotificationGroupMessage::ToggleDoNotDisturb => {
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
            QuietChange::None => Task::none(),
            QuietChange::Began { screen_off } => self
                .idle_timeouts_group
                .override_screen_off(screen_off)
                .map(Message::IdleTimeouts),
            QuietChange::Ended => {
                let released = self.notifications.release_held(&ToastContext::new(
                    &self.niri,
                    self.desktop_entries.get(),
                    &self.config,
                    &self.shell_settings,
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
