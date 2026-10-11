use std::{collections::HashMap, iter, mem, path::Path};

use iced::{
    Task,
    time::{Duration, Instant},
    window::Id,
};
use iced_exwlshell::reexport::ActivationTokenSink;
use quick_xml::escape::escape;
use tokio::{sync::mpsc, time::timeout};

use super::{
    DEFAULT_ACTION, NotificationLayer, ToastContext, WidgetToastRequests,
    reading_alert_toasts::{DismissedReadingAlert, ReadingAlertToastChanges, ToastSlotForAlert},
    toast_card::{self, CardIcons},
    toast_lifecycle::{
        Countdown, ExitPace, Notification, NotificationMessage, ToastOrigin, lifetime,
    },
    toast_placement::{ToastWindow, placement_for},
};
use crate::{
    config::{Config, LoadError},
    data::{
        notifications::model::{ExpireTimeout, NotificationData, Outbound, close_reason},
        severity::Severity,
        sink::sound,
        source::niri::Placement,
    },
    desktop_shell::{DesktopShell, Message},
    message_subscriptions::niri_action,
    state::{self, NotificationSettings, OverlaySettings},
    widget_groups::notification_group::{NotificationGroupMessage, NotificationToggle},
    widgets::{announced_level::LevelReading, reading_alert::ReadingAlert},
};

const CONFIG_ERROR_TOAST_ID: u32 = 0;

const ACTIVATION_TOKEN_GRACE: Duration = Duration::from_millis(300);

fn is_window_owned_action(key: &str) -> bool {
    matches!(key, "move" | "quit")
}

impl DesktopShell {
    pub(crate) fn update_notification(&mut self, msg: NotificationMessage) -> Task<Message> {
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
            ),
            &|output| panels.get(output).copied(),
        )
    }

    pub(crate) fn prune_notifications(&mut self) -> Task<Message> {
        self.notifications.prune();
        Task::none()
    }

    pub(crate) fn update_notification_group(
        &mut self,
        msg: NotificationGroupMessage,
    ) -> Task<Message> {
        match msg {
            NotificationGroupMessage::Expansion(menu) => {
                self.notification_group.expansion.update(menu);
                Task::none()
            }
            NotificationGroupMessage::Toggle(toggle) => self
                .notifications
                .toggle(toggle, &mut self.shell_settings.notifications),
        }
    }

    pub fn any_notification_transitioning(&self) -> bool {
        self.notifications.is_transitioning()
    }

    pub fn any_notification_expiring(&self) -> bool {
        self.notifications.has_running_countdown()
    }
}

impl NotificationLayer {
    pub fn adopt_sound_theme(&mut self, theme: Option<String>) {
        self.sound_theme = theme;
    }

    pub fn update(
        &mut self,
        message: NotificationMessage,
        context: &ToastContext,
        panel_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Message> {
        let handled = self.handle(message, context, panel_on);
        Task::batch([handled, self.release_orphaned_keyboard()])
    }

    fn handle(
        &mut self,
        message: NotificationMessage,
        context: &ToastContext,
        panel_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Message> {
        match message {
            NotificationMessage::Connected(handle) => {
                self.handle = Some(handle);
                Task::none()
            }
            NotificationMessage::Served(data) => self.serve(data, ToastOrigin::DbusClient, context),
            NotificationMessage::BodyClicked(id) => {
                self.activate_action(id, DEFAULT_ACTION, context, panel_on)
            }
            NotificationMessage::HoverChanged { id, hovered } => {
                if hovered {
                    self.hovered = Some(id);
                } else if self.hovered == Some(id) {
                    self.hovered = None;
                }
                self.sync_countdowns(Instant::now());
                Task::none()
            }
            NotificationMessage::Dismiss(id) => {
                self.remember_dismissed_reading_alert(id);
                self.close_all([id], close_reason::DISMISSED, context.config)
            }
            NotificationMessage::ActionClicked { id, key } => {
                self.activate_action(id, &key, context, panel_on)
            }
            NotificationMessage::DefaultActivated { id, token } => {
                self.invoke_default(id, token, context.config)
            }
            NotificationMessage::CloseRequested(id) => {
                self.close_all([id], close_reason::CLOSED_BY_CALL, context.config)
            }
            NotificationMessage::Tick => self.expire(context.config),
            NotificationMessage::Reply { id, message } => {
                self.update_reply(id, message, panel_on, context.config)
            }
            NotificationMessage::OpenFile(id) => self.open_previewed_file(id, context.config),
            NotificationMessage::CopyFilePath(id) => self.copy_previewed_file_path(id),
        }
    }

    fn previewed_file(&self, id: u32) -> Option<String> {
        self.showing()
            .find(|n| n.data.id == id)?
            .icons
            .previewed_file()
            .map(|file| file.to_string_lossy().into_owned())
    }

    fn open_previewed_file(&mut self, id: u32, config: &Config) -> Task<Message> {
        let Some(file) = self.previewed_file(id) else {
            return Task::none();
        };
        Task::batch([
            Task::done(Message::OpenUrl(file)),
            self.close_all([id], close_reason::DISMISSED, config),
        ])
    }

    fn copy_previewed_file_path(&self, id: u32) -> Task<Message> {
        self.previewed_file(id)
            .map_or_else(Task::none, |file| Task::done(Message::CopyText(file)))
    }

    pub fn show_widget_toasts(
        &mut self,
        requests: WidgetToastRequests,
        context: &ToastContext,
    ) -> Task<Message> {
        let alert_toasts = self.show_reading_alerts(&requests.alerts, context);
        let previous_levels = mem::replace(&mut self.previous_levels, requests.levels.clone());
        let level_toasts = requests
            .levels
            .into_iter()
            .filter(|reading| reading.changed_since(&previous_levels))
            .map(|reading| self.announce_level(reading, context));
        Task::batch(iter::once(alert_toasts).chain(level_toasts))
    }

    fn show_reading_alerts(
        &mut self,
        active_alerts: &[ReadingAlert<Message>],
        context: &ToastContext,
    ) -> Task<Message> {
        if context.shell_settings.notifications.do_not_disturb {
            return Task::none();
        }
        let changes = self.plan_reading_alert_toasts(active_alerts);
        if changes.is_empty() {
            return Task::none();
        }
        self.apply_reading_alert_toast_changes(changes, context)
    }

    fn announce_level(
        &mut self,
        reading: LevelReading<Message>,
        context: &ToastContext,
    ) -> Task<Message> {
        let toast_id = self
            .showing()
            .find(|n| n.announces_level_of(&reading.hover_target))
            .map_or_else(|| self.fresh_reading_alert_toast_id(), |n| n.data.id);
        self.serve(
            Box::new(reading.toast_notification_data(toast_id)),
            ToastOrigin::LevelAnnouncement(Box::new(reading)),
            context,
        )
    }

    pub fn show_config_error(
        &mut self,
        path: &Path,
        error: &LoadError,
        context: &ToastContext,
    ) -> Task<Message> {
        let data = NotificationData {
            body: Some(format!("<pre>{}</pre>", escape(error.to_string()))),
            severity: Severity::Critical,
            expire_timeout: ExpireTimeout::Never,
            ..NotificationData::from_shell(
                CONFIG_ERROR_TOAST_ID,
                format!("Invalid configuration in {}", path.display()),
            )
        };
        self.serve(Box::new(data), ToastOrigin::ConfigError, context)
    }

    pub fn withdraw_config_error(&mut self, config: &Config) -> Task<Message> {
        self.close_all(
            [CONFIG_ERROR_TOAST_ID],
            close_reason::CLOSED_BY_CALL,
            config,
        )
    }

    fn apply_reading_alert_toast_changes(
        &mut self,
        changes: ReadingAlertToastChanges<Message>,
        context: &ToastContext,
    ) -> Task<Message> {
        self.dismissed_reading_alerts.retain(|dismissed| {
            !changes
                .dismissals_to_forget
                .contains(&dismissed.hover_target)
        });
        let mut tasks = vec![self.close_all(
            changes.toast_ids_to_close,
            close_reason::CLOSED_BY_CALL,
            context.config,
        )];
        tasks.extend(changes.alerts_to_show.into_iter().map(|to_show| {
            let toast_id = match to_show.toast_slot {
                ToastSlotForAlert::ReplaceShownToast(toast_id) => toast_id,
                ToastSlotForAlert::OpenNewToast => self.fresh_reading_alert_toast_id(),
            };
            self.serve(
                Box::new(to_show.alert.toast_notification_data(toast_id)),
                ToastOrigin::ReadingAlert(Box::new(to_show.alert)),
                context,
            )
        }));
        Task::batch(tasks)
    }

    fn remember_dismissed_reading_alert(&mut self, toast_id: u32) {
        let dismissed = self
            .showing()
            .find(|n| n.data.id == toast_id)
            .and_then(Notification::reading_alert)
            .map(|alert| DismissedReadingAlert {
                hover_target: alert.hover_target.clone(),
                dismissed_at_severity: alert.severity,
            });
        self.dismissed_reading_alerts.extend(dismissed);
    }

    fn serve(
        &mut self,
        data: Box<NotificationData>,
        origin: ToastOrigin,
        context: &ToastContext,
    ) -> Task<Message> {
        let ToastContext {
            placement,
            entries,
            config,
            shell_settings,
        } = context;
        let notifications = &shell_settings.notifications;
        if notifications.do_not_disturb {
            return Task::none();
        }
        let id = data.id;
        let mut tasks = Vec::new();
        if let Some(window) = data.window_id_hint {
            let superseded: Vec<u32> = self
                .showing()
                .filter(|n| n.data.id != id)
                .filter(|n| n.data.window_id_hint == Some(window))
                .filter(|n| n.data.app_name == data.app_name)
                .map(|n| n.data.id)
                .collect();
            tasks.push(self.close_all(superseded, close_reason::UNDEFINED, config));
        }
        let icons = origin.glyph().map_or_else(
            || toast_card::resolve_icons(&data, *entries),
            CardIcons::of_glyph,
        );
        let now = Instant::now();
        let countdown = lifetime(
            data.expire_timeout,
            data.severity,
            &config.notification_toasts.lifetime,
        )
        .map(|remaining| Countdown::started(remaining, now));
        let window = ToastWindow::resolve(&data, placement);
        let place = placement_for(window, placement, shell_settings.overlays, config);
        let shown = self
            .live
            .iter_mut()
            .find(|n| n.data.id == id && !n.is_withdrawing());
        if let Some(shown) = shown {
            shown.replace_content(*data, origin, icons, countdown, place);
        } else {
            if notifications.sound && !data.suppress_sound {
                tasks.push(sound::play(&data, self.sound_theme.as_deref()));
            }
            self.live.retain(|n| n.data.id != id);
            let seq = self.next_seq;
            self.next_seq += 1;
            self.live.push(Notification::appearing(
                *data, origin, icons, countdown, seq, place, config,
            ));
        }
        self.sync_countdowns(now);
        Task::batch(tasks)
    }

    pub fn on_niri_changed(
        &mut self,
        placement: &Placement,
        config: &Config,
        overlays: OverlaySettings,
    ) -> Task<Message> {
        let stale: Vec<u32> = self
            .showing()
            .filter(|n| is_stale(n, placement))
            .map(|n| n.data.id)
            .collect();
        let closed = self.close_all(stale, close_reason::UNDEFINED, config);
        self.reposition(placement, config, overlays);
        Task::batch([closed, self.release_orphaned_keyboard()])
    }

    pub fn prune(&mut self) {
        let now = Instant::now();
        self.live.retain(|n| !n.has_faded(now));
    }

    fn expire(&mut self, config: &Config) -> Task<Message> {
        let now = Instant::now();
        let expired: Vec<u32> = self
            .showing()
            .filter(|n| n.countdown.is_some_and(|c| c.has_elapsed(now)))
            .map(|n| n.data.id)
            .collect();
        let closed = self.close_all(expired, close_reason::EXPIRED, config);
        self.prune();
        closed
    }

    pub(super) fn sync_countdowns(&mut self, now: Instant) {
        let hovered = self.hovered;
        self.live.iter_mut().for_each(|notif| {
            let held = hovered == Some(notif.data.id) || !notif.reply_draft.is_empty();
            notif.countdown = notif.countdown.map(|countdown| {
                if held {
                    countdown.paused(now)
                } else {
                    countdown.resumed(now)
                }
            });
        });
    }

    pub(super) fn withdraw_showing(
        &mut self,
        candidates: impl IntoIterator<Item = u32>,
        pace: ExitPace,
        config: &Config,
    ) -> Vec<u32> {
        let now = Instant::now();
        let closing: Vec<u32> = candidates
            .into_iter()
            .filter(|id| self.showing().any(|n| n.data.id == *id))
            .collect();
        self.live
            .iter_mut()
            .filter(|n| closing.contains(&n.data.id))
            .for_each(|n| n.withdraw(now, pace, config));
        self.prune_hover();
        closing
    }

    fn close_all(
        &mut self,
        candidates: impl IntoIterator<Item = u32>,
        reason: u32,
        config: &Config,
    ) -> Task<Message> {
        let closed = self
            .withdraw_showing(candidates, reason.into(), config)
            .into_iter()
            .filter(|id| self.sent_by_dbus_client(*id))
            .map(|id| Outbound::Closed { id, reason })
            .collect();
        self.emit_in_order(closed)
    }

    fn prune_hover(&mut self) {
        let hovered = self.hovered;
        self.hovered = hovered.filter(|id| self.showing().any(|n| n.data.id == *id));
    }

    fn activate_action(
        &mut self,
        id: u32,
        key: &str,
        context: &ToastContext,
        panel_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Message> {
        let placement = &context.placement;
        if let Some(shell_message) = self.shell_message_for(id, key) {
            self.remember_dismissed_reading_alert(id);
            return Task::batch([
                Task::done(shell_message),
                self.close_all([id], close_reason::DISMISSED, context.config),
            ]);
        }
        if key == DEFAULT_ACTION && self.declares_default_action(id) {
            return self.hand_default_action_back_to_sender(id, panel_on);
        }

        let survives_action_invocation = self.showing().any(|n| n.data.id == id && n.data.resident);
        let window_action = self
            .window_of(id, placement)
            .and_then(|window| action_on_window(key, window, placement));
        let activation = match window_action {
            Some(task) => task,
            None if key == DEFAULT_ACTION => Task::none(),
            None => self.emit_in_order(vec![Outbound::ActionInvoked {
                id,
                key: key.to_owned(),
            }]),
        };

        if survives_action_invocation {
            activation
        } else {
            Task::batch([
                activation,
                self.close_all([id], close_reason::DISMISSED, context.config),
            ])
        }
    }

    fn hand_default_action_back_to_sender(
        &self,
        id: u32,
        panel_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Message> {
        let toast_surface = self
            .showing()
            .find(|n| n.data.id == id)
            .and_then(|n| n.place.output.as_deref())
            .and_then(panel_on);
        let Some(toast_surface) = toast_surface else {
            return Task::done(Message::Notification(
                NotificationMessage::DefaultActivated { id, token: None },
            ));
        };

        let (sender, mut receiver) = mpsc::channel(1);
        let sink = ActivationTokenSink::new(move |token| {
            let _ = sender.try_send(token);
        });

        Task::batch([
            Task::done(Message::RequestActivationToken {
                id: toast_surface,
                sink,
            }),
            Task::future(async move {
                timeout(ACTIVATION_TOKEN_GRACE, receiver.recv())
                    .await
                    .ok()
                    .flatten()
            })
            .map(move |token| {
                Message::Notification(NotificationMessage::DefaultActivated { id, token })
            }),
        ])
    }

    fn invoke_default(&mut self, id: u32, token: Option<String>, config: &Config) -> Task<Message> {
        let survives_action_invocation = self.showing().any(|n| n.data.id == id && n.data.resident);
        let mut outbound: Vec<Outbound> = token
            .map(|token| Outbound::ActivationToken { id, token })
            .into_iter()
            .collect();
        outbound.push(Outbound::ActionInvoked {
            id,
            key: DEFAULT_ACTION.to_owned(),
        });

        if survives_action_invocation {
            return self.emit_in_order(outbound);
        }

        outbound.extend(
            self.withdraw_showing([id], ExitPace::Dismissed, config)
                .into_iter()
                .map(|id| Outbound::Closed {
                    id,
                    reason: close_reason::DISMISSED,
                }),
        );
        self.emit_in_order(outbound)
    }

    pub fn toggle(
        &mut self,
        toggle: NotificationToggle,
        notifications: &mut NotificationSettings,
    ) -> Task<Message> {
        match toggle {
            NotificationToggle::DoNotDisturb => {
                notifications.do_not_disturb = !notifications.do_not_disturb;
                self.sync_dnd(notifications);
                Task::future(state::shell_settings::save_do_not_disturb(
                    notifications.do_not_disturb,
                ))
                .discard()
            }
            NotificationToggle::Sound => {
                notifications.sound = !notifications.sound;
                Task::future(state::shell_settings::save_sound(notifications.sound)).discard()
            }
        }
    }

    fn sync_dnd(&mut self, notifications: &NotificationSettings) {
        if notifications.do_not_disturb {
            self.live.clear();
            self.hovered = None;
        }
    }

    pub(super) fn emit_in_order(&self, outbound: Vec<Outbound>) -> Task<Message> {
        match &self.handle {
            Some(handle) if !outbound.is_empty() => {
                let sender = handle.sender();
                Task::future(async move {
                    for signal in outbound {
                        let _ = sender.send(signal).await;
                    }
                })
                .discard()
            }
            _ => Task::none(),
        }
    }

    fn reposition(&mut self, placement: &Placement, config: &Config, overlays: OverlaySettings) {
        self.live.iter_mut().for_each(|notif| {
            notif.place = placement_for(notif.place.window, placement, overlays, config);
        });
    }
}

fn action_on_window(key: &str, window: u64, placement: &Placement) -> Option<Task<Message>> {
    match key {
        DEFAULT_ACTION => Some(niri_action(niri_ipc::Action::FocusWindow { id: window })),
        "quit" => Some(niri_action(niri_ipc::Action::CloseWindow {
            id: Some(window),
        })),
        "move" => Some(
            placement
                .active_workspace_for_move()
                .map_or_else(Task::none, |target| {
                    niri_action(niri_ipc::Action::MoveWindowToWorkspace {
                        window_id: Some(window),
                        reference: niri_ipc::WorkspaceReferenceArg::Id(target),
                        focus: true,
                    })
                }),
        ),
        _ => None,
    }
}

fn is_stale(notif: &Notification, placement: &Placement) -> bool {
    let window = notif.place.window;
    let window_owned = notif
        .data
        .actions
        .iter()
        .all(|a| is_window_owned_action(&a.key));
    window != ToastWindow::Unanchored && window_owned && window.open_in(placement).is_none()
}
