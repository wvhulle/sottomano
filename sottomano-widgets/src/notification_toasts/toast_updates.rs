mod shell_failure_toasts;
mod toast_action_invocation;
mod widget_toast_serving;

use iced::{
    Task,
    futures::{StreamExt, stream},
    time::Instant,
    window::Id,
};
use sottomano_desktop_sinks::desktop_notifications::{
    alert_sound_player::SoundTheme, alert_sound_policy::ToastSlot,
    freedesktop_notification_signals::notification_signal_sink,
};
use sottomano_desktop_sources::{
    desktop_notifications::notification_data::{Outbound, Residency, close_reason},
    desktop_windows_apps::window_edge_placement::Placement,
};
use sottomano_toml_config::Config;

use super::{
    DEFAULT_ACTION, HoveredToast, NotificationLayer, SignalConnection, ToastContext,
    toast_card::toast_card_icons::{self, CardIcons},
    toast_chimes::arrival_chime,
    toast_lifecycle::{
        BodyClick, CardStart, EventSource, ExitPace, Notification, ToastContent, ToastKind,
        ToastPresence,
        toast_countdown::{Countdown, lifetime},
    },
    toast_placement::{AnchoredWindowFocus, ToastWindow, delegated_sender_window, placement_for},
};
use crate::{
    hover_cards::{HoverCardMessage, PointerPresence, PopupMessage},
    notification_toasts::{
        NotificationSettings, ToastLayerHost,
        toast_lifecycle::{FadeState, ToastSender, toast_countdown::CountdownExpiry},
        toast_messages::NotificationMessage,
    },
    panel_menus::agenda_menu::quiet_schedule_group::quiet_window::QuietPhase,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoppedChangeWatch {
    ConfigFile,
    FirefoxProfile,
}

impl StoppedChangeWatch {
    const fn stale_consequence(self) -> &'static str {
        match self {
            StoppedChangeWatch::ConfigFile => {
                "Edits to the config file are not picked up until the shell restarts."
            }
            StoppedChangeWatch::FirefoxProfile => {
                "Firefox favicons stop refreshing until the shell restarts."
            }
        }
    }
}

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
                self.signal_connection = SignalConnection::Connected(signals);
                Task::none()
            }
            NotificationMessage::Served(data) => self.serve(
                ToastContent {
                    data: *data,
                    kind: ToastKind::Event {
                        at: context.clock.wall,
                        source: EventSource::DbusClient,
                    },
                    body_click: BodyClick::DbusDefaultAction,
                },
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
                    PointerPresence::PointedAt => self.hovered = HoveredToast::Hovered(id),
                    PointerPresence::NotPointedAt if self.hovered == HoveredToast::Hovered(id) => {
                        self.hovered = HoveredToast::Unhovered;
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
            NotificationMessage::CardsSettled { toast_ids } => self.ring_settled_chimes(&toast_ids),
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

    fn serve(&mut self, toast: ToastContent<Routed>, context: &ToastContext) -> Task<Routed> {
        let ToastContext {
            placement,
            config,
            notifications,
            quiet_phase,
            clock,
        } = context;
        if *quiet_phase == QuietPhase::Active {
            return self.hold_while_quiet(toast);
        }
        let data = &toast.data;
        let id = data.id;
        let mut tasks = Vec::new();
        if let Some(window) = delegated_sender_window(data, placement) {
            let superseded: Vec<u32> = self
                .showing()
                .filter(|shown| shown.data.id != id)
                .filter(|shown| shown.place.window == ToastWindow::Anchored(window))
                .filter(|shown| shown.data.app_name == data.app_name)
                .map(|shown| shown.data.id)
                .collect();
            tasks.push(self.close_all(superseded, close_reason::UNDEFINED, config, clock.instant));
        }
        let icons = toast
            .kind
            .glyph()
            .map_or_else(|| toast_card_icons::resolve_icons(data), CardIcons::from);
        let now = clock.instant;
        let countdown = Countdown::counting(
            lifetime(
                data.expire_timeout,
                data.severity,
                &config.notification_toasts.lifetime,
            ),
            now,
        );
        let window = ToastWindow::resolve(data, placement);
        let place = placement_for(window, placement, *notifications, config);
        let stacked_predecessors = self.stacked_predecessors(data);
        let toast_slot = self
            .showing()
            .filter(|shown| shown.data.id == id || stacked_predecessors.contains(&shown.data.id))
            .map(|shown| shown.data.severity)
            .max()
            .map_or(ToastSlot::Vacant, |shown_severity| ToastSlot::Occupied {
                shown_severity,
            });
        let chime = arrival_chime(data, toast_slot, notifications.alert_sound);
        tasks.push(self.close_all(
            stacked_predecessors,
            close_reason::UNDEFINED,
            config,
            clock.instant,
        ));
        let shown = self.live.iter_mut().find(|live_notification| {
            live_notification.data.id == id && live_notification.presence() == ToastPresence::Shown
        });
        if let Some(shown) = shown {
            shown.replace_content(toast, icons, countdown, place);
            tasks.push(self.play_chime(chime));
        } else {
            self.live
                .retain(|live_notification| live_notification.data.id != id);
            let seq = self.next_seq;
            self.next_seq += 1;
            self.live.push(Notification::appearing(
                toast,
                CardStart {
                    icons,
                    countdown,
                    chime,
                },
                seq,
                place,
                config,
                now,
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
        let returned_to: Vec<u32> = self
            .showing()
            .filter(|notif| notif.data.residency == Residency::Transient)
            .filter(|notif| notif.place.window_focus != AnchoredWindowFocus::Focused)
            .filter(|notif| {
                notif.place.window.focus_within(placement) == AnchoredWindowFocus::Focused
            })
            .map(|notif| notif.data.id)
            .collect();
        let closed = self.close_all(stale, close_reason::UNDEFINED, config, now);
        let dismissed = self.close_all(returned_to, close_reason::DISMISSED, config, now);
        self.reposition(placement, config, notifications);
        Task::batch([closed, dismissed, self.release_orphaned_keyboard()])
    }

    pub fn prune(&mut self, now: Instant) {
        self.live
            .retain(|toast| toast.fade_state(now) == FadeState::Visible);
    }

    fn expire(&mut self, context: &ToastContext) -> Task<Routed> {
        let now = context.clock.instant;
        let expired: Vec<u32> = self
            .showing()
            .filter(|notification| {
                notification.countdown.expiry_at(now) == CountdownExpiry::Elapsed
            })
            .map(|notification| notification.data.id)
            .collect();
        let closed = self.close_all(expired, close_reason::EXPIRED, context.config, now);
        self.prune(now);
        closed
    }

    pub(super) fn sync_countdowns(&mut self, now: Instant) {
        let hovered = self.hovered;
        self.live.iter_mut().for_each(|notification| {
            let held = hovered == HoveredToast::Hovered(notification.data.id)
                || !notification.reply_draft.is_empty();
            notification.countdown = if held {
                notification.countdown.paused(now)
            } else {
                notification.countdown.resumed(now)
            };
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
            .filter(|id| self.showing().any(|toast| toast.data.id == *id))
            .collect();
        self.live
            .iter_mut()
            .filter(|toast| closing.contains(&toast.data.id))
            .for_each(|toast| toast.withdraw(now, pace, config));
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
            .filter(|id| self.toast_sender_by_id(*id) == Some(ToastSender::DbusClient))
            .map(|id| Outbound::Closed { id, reason })
            .collect();
        self.emit_in_order(closed)
    }

    fn prune_hover(&mut self) {
        if let HoveredToast::Hovered(id) = self.hovered
            && !self.showing().any(|toast| toast.data.id == id)
        {
            self.hovered = HoveredToast::Unhovered;
        }
    }

    pub(super) fn emit_in_order(&self, outbound: Vec<Outbound>) -> Task<Routed> {
        match &self.signal_connection {
            SignalConnection::Connected(signals) if !outbound.is_empty() => Task::future(
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
