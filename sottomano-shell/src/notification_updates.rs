use std::collections::HashMap;

use iced::{Task, window::Id};
use sottomano_widgets::{
    group_menus::group_menu_state::{GroupMenuMessage, PanelRaise},
    notification_toasts::ToastContext,
    toast_messages::NotificationMessage,
    widget_groups::{
        notification_group,
        notification_group::NotificationGroupMessage,
        quiet_schedule_group::{
            QuietChange, QuietScheduleMessage, QuietUpdate, quiet_window::QuietPhase,
        },
        widget_group_content::GroupToastRequests,
    },
};

use crate::{
    desktop_shell::DesktopShell,
    panels::{composed_menu_topics, composed_standalone_groups},
    shell_message::{Message, ShellInteraction},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AlertRescanNeed {
    Rescans,
    Skips,
}

pub(crate) fn alert_rescan_need(message: &Message) -> AlertRescanNeed {
    match message {
        Message::HoverCard(_)
        | Message::GroupMenuState(
            GroupMenuMessage::InspectRow { .. }
            | GroupMenuMessage::IntendRevealMenuGroup { .. }
            | GroupMenuMessage::CancelRevealIntent { .. }
            | GroupMenuMessage::RevealIntentElapsed
            | GroupMenuMessage::RevealMenuGroup { .. }
            | GroupMenuMessage::ToggleMenuGroup { .. }
            | GroupMenuMessage::MenuScrolled { .. }
            | GroupMenuMessage::ItemListScrolled { .. },
        )
        | Message::Redraw
        | Message::HoverCardFadeElapsed
        | Message::HoverCardHideElapsed
        | Message::PointerLeft
        | Message::SurfacePressed(_)
        | Message::TypingUnderCards(_) => AlertRescanNeed::Skips,
        _ => AlertRescanNeed::Rescans,
    }
}

pub(crate) struct GroupAlertScan {
    pub toast_requests: GroupToastRequests<ShellInteraction>,
    pub panel_raises: Vec<PanelRaise>,
}

impl DesktopShell {
    pub(crate) fn scan_group_alerts(&self) -> GroupAlertScan {
        let locale = self.config.calendar.locale;
        let group_menus: Vec<_> = composed_menu_topics()
            .map(|topic| self.group_menu_of(topic, self.hovered_output()))
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
        let panel_raises = group_menus
            .iter()
            .flat_map(|group_menu| group_menu.panel_raises(promotion_severity, self.clock))
            .collect();
        GroupAlertScan {
            toast_requests,
            panel_raises,
        }
    }

    pub fn update_notification(&mut self, msg: NotificationMessage) -> Task<Message> {
        let toast_surfaces: HashMap<String, Id> = self
            .notification_toast_surfaces()
            .map(|(id, output)| (output.to_owned(), id))
            .collect();
        self.notifications
            .update(
                msg,
                &ToastContext::new(
                    &self.compositor,
                    self.desktop_entries.get(),
                    &self.config,
                    self.widget_groups.notification_settings(),
                    self.widget_groups.quiet_schedule_group.quiet_phase(),
                    self.clock,
                ),
                &|output| toast_surfaces.get(output).copied(),
            )
            .map(Message::from)
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
                .idle_pause_groups
                .override_screen_off(Some(timeout))
                .map(Message::IdleTimeouts),
            QuietChange::ScreenOffFollowsIdleTimer => self
                .widget_groups
                .idle_pause_groups
                .override_screen_off(None)
                .map(Message::IdleTimeouts),
            QuietChange::Ended => {
                let released = self
                    .notifications
                    .release_held(&ToastContext::new(
                        &self.compositor,
                        self.desktop_entries.get(),
                        &self.config,
                        self.widget_groups.notification_settings(),
                        QuietPhase::Inactive,
                        self.clock,
                    ))
                    .map(Message::from);
                Task::batch([
                    released,
                    self.widget_groups
                        .idle_pause_groups
                        .override_screen_off(None)
                        .map(Message::IdleTimeouts),
                ])
            }
        }
    }
}
