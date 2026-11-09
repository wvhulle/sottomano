use std::{borrow::Cow, iter, mem, path::Path};

use iced::{
    Task,
    futures::{StreamExt, stream},
    time::Instant,
    window::Id,
};
use quick_xml::escape::escape;
use sottomano_desktop_sinks::{
    compositor_window_requests::compositor_requests::{
        CompositorRequest, CompositorRequestFailure,
    },
    desktop_notifications::{
        alert_sound_player::{self, SoundTheme},
        freedesktop_notification_signals::notification_signal_sink,
    },
};
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::{
        NotificationData, Outbound, Residency, SoundHint, close_reason,
    },
    desktop_windows_apps::window_edge_placement::Placement,
};
use sottomano_toml_config::{Config, LoadError, severity::Severity};

use super::{
    DEFAULT_ACTION, NotificationLayer, SnoozedOngoingToast, ToastContext,
    toast_card::{self, CardIcons},
    toast_lifecycle::{
        Countdown, EventSource, ExitPace, Notification, ToastContent, ToastKind, ToastPresence,
        lifetime,
    },
    toast_placement::{ToastWindow, placement_for},
};
use crate::{
    background_work::compositor_request,
    hover_cards::{HoverCardMessage, PointerPresence, PopupMessage},
    layer_surfaces::{ActivationTokenOutcome, activation_token_request},
    notification_toasts::{
        NotificationSettings, ToastLayerHost,
        toast_lifecycle::{CountdownExpiry, FadeState, ToastSender},
    },
    toast_messages::NotificationMessage,
    widget_groups::{
        notification_group::AlertSound, quiet_schedule_group::quiet_window::QuietPhase,
        widget_group_content::GroupToastRequests,
    },
    widgets::{
        announced_level::{LevelReading, LevelReadingChange},
        ongoing_toast::{OngoingKey, OngoingToast},
    },
};

impl<Routed: ToastLayerHost> NotificationLayer<Routed> {
    pub fn adopt_sound_theme(&mut self, theme: SoundTheme) {
        self.sound_theme = theme;
    }

    pub fn update(
        &mut self,
        message: NotificationMessage,
        context: &ToastContext,
        toast_surface_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Routed> {
        let handled = self.handle(message, context, toast_surface_on);
        Task::batch([handled, self.release_orphaned_keyboard()])
    }

    fn handle(
        &mut self,
        message: NotificationMessage,
        context: &ToastContext,
        toast_surface_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Routed> {
        match message {
            NotificationMessage::Connected(signals) => {
                self.signals = Some(signals);
                Task::none()
            }
            NotificationMessage::Served(data) => self.serve(
                ToastContent::from_dbus_client(*data, context.clock.wall),
                context,
            ),
            NotificationMessage::BodyClicked(id) => {
                self.activate_action(id, DEFAULT_ACTION, context, toast_surface_on)
            }
            NotificationMessage::HoverChanged {
                id,
                pointer_presence,
            } => {
                match pointer_presence {
                    PointerPresence::PointedAt => self.hovered = Some(id),
                    PointerPresence::NotPointedAt if self.hovered == Some(id) => {
                        self.hovered = None;
                    }
                    PointerPresence::NotPointedAt => {}
                }
                self.sync_countdowns(context.clock.instant);
                Task::none()
            }
            NotificationMessage::Dismiss(id) => {
                self.snooze_ongoing_toast(id);
                self.close_all(
                    [id],
                    close_reason::DISMISSED,
                    context.config,
                    context.clock.instant,
                )
            }
            NotificationMessage::ActionClicked { id, key } => {
                self.activate_action(id, &key, context, toast_surface_on)
            }
            NotificationMessage::DefaultActivated { id, token } => {
                self.invoke_default(id, token, context)
            }
            NotificationMessage::CloseRequested(id) => Task::batch([
                self.drop_held(id),
                self.close_all(
                    [id],
                    close_reason::CLOSED_BY_CALL,
                    context.config,
                    context.clock.instant,
                ),
            ]),
            NotificationMessage::ExpiryDue => self.expire(context),
            NotificationMessage::Reply { id, message } => {
                self.update_reply(id, message, toast_surface_on, context)
            }
            NotificationMessage::OpenFile(id) => self.open_previewed_file(id, context),
            NotificationMessage::CopyFilePath(id) => self.copy_previewed_file_path(id),
            NotificationMessage::CardsLaidOut {
                output,
                toast_cards,
            } => Task::done(Routed::from(PopupMessage::HoverCard(
                HoverCardMessage::ToastCardsLaidOut {
                    output,
                    toast_cards,
                },
            ))),
        }
    }

    fn previewed_file(&self, id: u32) -> Option<String> {
        self.showing()
            .find(|n| n.data.id == id)?
            .icons
            .previewed_file()
            .map(|file| file.to_string_lossy().into_owned())
    }

    fn open_previewed_file(&mut self, id: u32, context: &ToastContext) -> Task<Routed> {
        let Some(file) = self.previewed_file(id) else {
            return Task::none();
        };
        Task::batch([
            Task::done(Routed::from(PopupMessage::OpenUrl(file))),
            self.close_all(
                [id],
                close_reason::DISMISSED,
                context.config,
                context.clock.instant,
            ),
        ])
    }

    fn copy_previewed_file_path(&self, id: u32) -> Task<Routed> {
        self.previewed_file(id).map_or_else(Task::none, |file| {
            Task::done(Routed::from(PopupMessage::CopyText(file)))
        })
    }

    pub fn show_widget_toasts(
        &mut self,
        requests: GroupToastRequests<Routed>,
        context: &ToastContext,
    ) -> Task<Routed> {
        let ongoing_toasts = self.show_ongoing_toasts(requests.ongoing_toasts, context);
        let previous_levels = mem::replace(&mut self.previous_levels, requests.levels.clone());
        let level_toasts: Vec<Task<Routed>> = requests
            .levels
            .into_iter()
            .filter(|reading| reading.change_since(&previous_levels) == LevelReadingChange::Changed)
            .map(|reading| self.announce_level(reading, context))
            .collect();
        Task::batch(iter::once(ongoing_toasts).chain(level_toasts))
    }

    fn show_ongoing_toasts(
        &mut self,
        active: Vec<OngoingToast<Routed>>,
        context: &ToastContext,
    ) -> Task<Routed> {
        if context.quiet_phase == QuietPhase::Active {
            return Task::none();
        }
        self.snoozed.retain(|snoozed| {
            active
                .iter()
                .any(|toast| toast.key == snoozed.key && toast.severity <= snoozed.snoozed_at)
        });
        let cleared: Vec<u32> = self
            .showing()
            .filter(|n| {
                matches!(&n.kind, ToastKind::Ongoing { key: shown, .. }
                    if matches!(shown, OngoingKey::Reading(_))
                        && !active.iter().any(|toast| toast.key == *shown))
            })
            .map(|n| n.data.id)
            .collect();
        let closed = self.close_all(
            cleared,
            close_reason::CLOSED_BY_CALL,
            context.config,
            context.clock.instant,
        );
        let to_raise: Vec<OngoingToast<Routed>> = active
            .into_iter()
            .filter(|toast| !self.snoozed.iter().any(|snoozed| snoozed.key == toast.key))
            .filter(|toast| {
                !self.showing().any(|notif| {
                    notif.ongoing_key() == Some(&toast.key)
                        && notif.data.severity >= toast.severity
                        && notif.data.body == toast.body
                        && notif.data.progress == toast.progress
                })
            })
            .collect();
        let raised: Vec<Task<Routed>> = to_raise
            .into_iter()
            .map(|toast| self.serve_ongoing(toast, context))
            .collect();
        Task::batch(iter::once(closed).chain(raised))
    }

    fn serve_ongoing(
        &mut self,
        mut toast: OngoingToast<Routed>,
        context: &ToastContext,
    ) -> Task<Routed> {
        let shown = self
            .showing()
            .find(|notif| notif.ongoing_key() == Some(&toast.key))
            .map(|n| (n.data.id, n.ongoing_since()));
        let toast_id = match shown {
            Some((id, since)) => {
                toast.since = since.unwrap_or(toast.since);
                id
            }
            None => self.fresh_shell_toast_id(),
        };
        self.serve(ToastContent::ongoing(toast, toast_id), context)
    }

    fn announce_level(
        &mut self,
        reading: LevelReading<Routed>,
        context: &ToastContext,
    ) -> Task<Routed> {
        let toast_id = self
            .showing()
            .find(|notif| notif.announced_target() == Some(&reading.hover_target))
            .map_or_else(|| self.fresh_shell_toast_id(), |n| n.data.id);
        self.serve(
            ToastContent::level_announcement(reading, toast_id, context.clock.wall),
            context,
        )
    }

    pub fn show_config_error(
        &mut self,
        path: &Path,
        error: &LoadError,
        context: &ToastContext,
    ) -> Task<Routed> {
        let message = escape(error.message()).into_owned();
        let body = match error.offending_line() {
            Some(line) => format!(
                "{message}<pre>{} {}<u>{}</u>{}</pre>",
                line.number,
                escape(line.before),
                escape(line.offending),
                escape(line.after),
            ),
            None => message,
        };
        self.serve_ongoing(
            OngoingToast {
                widget_name: "Configuration",
                key: OngoingKey::ConfigFile,
                severity: Severity::Critical,
                summary_text: Cow::Owned(format!("Invalid configuration in {}", path.display())),
                body: Some(body),
                progress: None,
                since: context.clock.wall,
                on_body_clicked: Routed::from(PopupMessage::OpenUrl(path.display().to_string())),
                action_buttons: Vec::new(),
            },
            context,
        )
    }

    pub fn show_compositor_failure(
        &mut self,
        failure: &CompositorRequestFailure,
        context: &ToastContext,
    ) -> Task<Routed> {
        let toast_id = self.fresh_shell_toast_id();
        let data = NotificationData {
            severity: Severity::Warning,
            body: Some(format!(
                "{}<pre>{}</pre>",
                escape(failure.reason()),
                escape(failure.request())
            )),
            ..NotificationData::from_shell(toast_id, failure.summary())
        };
        self.serve(
            ToastContent::shell_failure(
                data,
                Routed::from(PopupMessage::CopyText(failure.to_string())),
                context.clock.wall,
            ),
            context,
        )
    }

    pub fn show_bus_failure(
        &mut self,
        failure: &zbus::Error,
        context: &ToastContext,
    ) -> Task<Routed> {
        let toast_id = self.fresh_shell_toast_id();
        let data = NotificationData {
            severity: Severity::Critical,
            body: Some(format!(
                "Widgets that read D-Bus stay empty.<pre>{}</pre>",
                escape(failure.to_string())
            )),
            ..NotificationData::from_shell(toast_id, "Could not connect to the D-Bus buses")
        };
        self.serve(
            ToastContent::shell_failure(
                data,
                Routed::from(PopupMessage::CopyText(failure.to_string())),
                context.clock.wall,
            ),
            context,
        )
    }

    pub fn withdraw_config_error(&mut self, config: &Config, now: Instant) -> Task<Routed> {
        let shown: Vec<u32> = self
            .showing()
            .filter(|notif| notif.ongoing_key() == Some(&OngoingKey::ConfigFile))
            .map(|n| n.data.id)
            .collect();
        self.close_all(shown, close_reason::CLOSED_BY_CALL, config, now)
    }

    fn hold_while_quiet(&mut self, toast: ToastContent<Routed>) {
        if !matches!(
            toast.kind,
            ToastKind::Event {
                source: EventSource::DbusClient,
                ..
            }
        ) {
            return;
        }
        self.held.retain(|held| held.id != toast.data.id);
        self.held.push(toast.data);
    }

    pub fn release_held(&mut self, context: &ToastContext) -> Task<Routed> {
        let held: Vec<NotificationData> = mem::take(&mut self.held);
        Task::batch(held.into_iter().map(|data| {
            self.serve(
                ToastContent::from_dbus_client(data, context.clock.wall),
                context,
            )
        }))
    }

    fn drop_held(&mut self, id: u32) -> Task<Routed> {
        let was_held = self.held.iter().any(|held| held.id == id);
        self.held.retain(|held| held.id != id);
        if was_held {
            self.emit_in_order(vec![Outbound::Closed {
                id,
                reason: close_reason::CLOSED_BY_CALL,
            }])
        } else {
            Task::none()
        }
    }

    fn snooze_ongoing_toast(&mut self, toast_id: u32) {
        let snoozed = self
            .showing()
            .find(|n| n.data.id == toast_id)
            .and_then(|n| match &n.kind {
                ToastKind::Ongoing { key, .. } => Some(SnoozedOngoingToast {
                    key: key.clone(),
                    snoozed_at: n.data.severity,
                }),
                ToastKind::Event { .. } => None,
            });
        self.snoozed.extend(snoozed);
    }

    fn serve(&mut self, toast: ToastContent<Routed>, context: &ToastContext) -> Task<Routed> {
        let ToastContext {
            placement,
            entries,
            config,
            notifications,
            quiet_phase,
            clock,
        } = context;
        if *quiet_phase == QuietPhase::Active {
            self.hold_while_quiet(toast);
            return Task::none();
        }
        let data = &toast.data;
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
            tasks.push(self.close_all(superseded, close_reason::UNDEFINED, config, clock.instant));
        }
        let icons = toast.kind.glyph().map_or_else(
            || toast_card::resolve_icons(data, *entries),
            CardIcons::from,
        );
        let now = clock.instant;
        let countdown = lifetime(
            data.expire_timeout,
            data.severity,
            &config.notification_toasts.lifetime,
        )
        .map(|remaining| Countdown::started(remaining, now));
        let window = ToastWindow::resolve(data, placement);
        let place = placement_for(window, placement, *notifications, config);
        let shown = self
            .live
            .iter_mut()
            .find(|notif| notif.data.id == id && notif.presence() == ToastPresence::Shown);
        if let Some(shown) = shown {
            shown.replace_content(toast, icons, countdown, place);
        } else {
            if notifications.alert_sound == AlertSound::Audible
                && data.sound_hint == SoundHint::Allowed
            {
                tasks.push(
                    Task::future(alert_sound_player::play(data, &self.sound_theme)).discard(),
                );
            }
            self.live.retain(|n| n.data.id != id);
            let seq = self.next_seq;
            self.next_seq += 1;
            self.live.push(Notification::appearing(
                toast, icons, countdown, seq, place, config, now,
            ));
        }
        self.sync_countdowns(now);
        Task::batch(tasks)
    }

    pub fn on_compositor_changed(
        &mut self,
        placement: &Placement,
        config: &Config,
        notifications: NotificationSettings,
        now: Instant,
    ) -> Task<Routed> {
        let stale: Vec<u32> = self
            .showing()
            .filter(|notif| {
                let window = notif.place.window;
                let window_owned = notif
                    .data
                    .actions
                    .iter()
                    .all(|action| matches!(action.key.as_str(), "move" | "quit"));
                window != ToastWindow::Unanchored
                    && window_owned
                    && window.open_in(placement).is_none()
            })
            .map(|notif| notif.data.id)
            .collect();
        let closed = self.close_all(stale, close_reason::UNDEFINED, config, now);
        self.reposition(placement, config, notifications);
        Task::batch([closed, self.release_orphaned_keyboard()])
    }

    pub fn prune(&mut self, now: Instant) {
        self.live
            .retain(|n| n.fade_state(now) == FadeState::Visible);
    }

    fn expire(&mut self, context: &ToastContext) -> Task<Routed> {
        let now = context.clock.instant;
        let expired: Vec<u32> = self
            .showing()
            .filter(|n| {
                n.countdown
                    .is_some_and(|c| c.expiry_at(now) == CountdownExpiry::Elapsed)
            })
            .map(|n| n.data.id)
            .collect();
        let closed = self.close_all(expired, close_reason::EXPIRED, context.config, now);
        self.prune(now);
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
        now: Instant,
    ) -> Vec<u32> {
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
        now: Instant,
    ) -> Task<Routed> {
        let closed = self
            .withdraw_showing(candidates, reason.into(), config, now)
            .into_iter()
            .filter(|id| self.sender_of(*id) == Some(ToastSender::DbusClient))
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
        toast_surface_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Routed> {
        let placement = &context.placement;
        if let Some(routed_message) = self.routed_message_for(id, key) {
            self.snooze_ongoing_toast(id);
            return Task::batch([
                Task::done(routed_message),
                self.close_all(
                    [id],
                    close_reason::DISMISSED,
                    context.config,
                    context.clock.instant,
                ),
            ]);
        }
        let declares_default_action = self.showing().any(|notif| {
            notif.data.id == id
                && notif
                    .data
                    .actions
                    .iter()
                    .any(|action| action.key == DEFAULT_ACTION)
        });
        if key == DEFAULT_ACTION && declares_default_action {
            return self.hand_default_action_back_to_sender(id, toast_surface_on);
        }

        let survives_action_invocation = self
            .showing()
            .any(|n| n.data.id == id && n.data.residency == Residency::Resident);
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
                self.close_all(
                    [id],
                    close_reason::DISMISSED,
                    context.config,
                    context.clock.instant,
                ),
            ])
        }
    }

    fn hand_default_action_back_to_sender(
        &self,
        id: u32,
        toast_surface_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Routed> {
        let toast_surface = self
            .showing()
            .find(|n| n.data.id == id)
            .and_then(|n| n.place.output.as_deref())
            .and_then(toast_surface_on);
        let Some(toast_surface) = toast_surface else {
            return Task::done(Routed::from(NotificationMessage::DefaultActivated {
                id,
                token: ActivationTokenOutcome::Unavailable,
            }));
        };

        let token_request = activation_token_request(toast_surface);
        Task::batch([
            Task::done(Routed::from(token_request.surface_request)),
            Task::future(token_request.token_arrival).map(move |token| {
                Routed::from(NotificationMessage::DefaultActivated { id, token })
            }),
        ])
    }

    fn invoke_default(
        &mut self,
        id: u32,
        token: ActivationTokenOutcome,
        context: &ToastContext,
    ) -> Task<Routed> {
        let survives_action_invocation = self
            .showing()
            .any(|n| n.data.id == id && n.data.residency == Residency::Resident);
        let action_invoked = Outbound::ActionInvoked {
            id,
            key: DEFAULT_ACTION.to_owned(),
        };
        let mut outbound = match token {
            ActivationTokenOutcome::Issued(token) => {
                vec![Outbound::ActivationToken { id, token }, action_invoked]
            }
            ActivationTokenOutcome::Unavailable => vec![action_invoked],
        };

        if survives_action_invocation {
            return self.emit_in_order(outbound);
        }

        outbound.extend(
            self.withdraw_showing(
                [id],
                ExitPace::Dismissed,
                context.config,
                context.clock.instant,
            )
            .into_iter()
            .map(|id| Outbound::Closed {
                id,
                reason: close_reason::DISMISSED,
            }),
        );
        self.emit_in_order(outbound)
    }

    pub(super) fn emit_in_order(&self, outbound: Vec<Outbound>) -> Task<Routed> {
        match &self.signals {
            Some(signals) if !outbound.is_empty() => Task::future(
                stream::iter(outbound)
                    .map(Ok)
                    .forward(notification_signal_sink(signals.clone())),
            )
            .discard(),
            _ => Task::none(),
        }
    }

    fn reposition(
        &mut self,
        placement: &Placement,
        config: &Config,
        notifications: NotificationSettings,
    ) {
        self.live.iter_mut().for_each(|notif| {
            notif.place = placement_for(notif.place.window, placement, notifications, config);
        });
    }
}

fn action_on_window<Routed: ToastLayerHost>(
    key: &str,
    window: u64,
    placement: &Placement,
) -> Option<Task<Routed>> {
    match key {
        DEFAULT_ACTION => Some(compositor_request(CompositorRequest::FocusWindow(window))),
        "quit" => Some(compositor_request(CompositorRequest::CloseWindow(window))),
        "move" => Some(
            placement
                .active_workspace_for_move()
                .map_or_else(Task::none, |target| {
                    compositor_request(CompositorRequest::MoveWindowToWorkspace {
                        window,
                        workspace: target,
                        follows_focus: true,
                    })
                }),
        ),
        _ => None,
    }
}
