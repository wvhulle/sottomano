pub mod body_markup;
pub mod expiry_countdown;
pub mod placement_policy;
pub mod reading_alert_reconciliation;
pub mod toast_anchor;
pub mod toast_card;
pub mod toast_field;
pub mod toast_lifecycle;

use std::{cmp::Reverse, path::Path};

use expiry_countdown::Countdown;
use iced::{
    Element, Padding, Task,
    theme::palette::Extended,
    time::{Duration, Instant},
    window::Id,
};
use iced_exwlshell::reexport::ActivationTokenSink;
use niri_ipc::state::EventStreamState;
use placement_policy::{placement_for, resolve_window_id};
use reading_alert_reconciliation::{
    DismissedReadingAlert, ReadingAlertToastChanges, ShownReadingAlertToast, ToastSlotForAlert,
    plan_reading_alert_toast_changes,
};
use toast_anchor::ToastSlot;
pub use toast_lifecycle::{Notification, NotificationMessage, ToastOrigin};
use tokio::{sync::mpsc, time::timeout};

use crate::{
    app::{Bar, Message},
    config::{Config, LoadError},
    data::{
        notifications::model::{
            ExpireTimeout, NotifHandle, NotificationData, Outbound, close_reason,
        },
        severity::Severity,
        sink::sound,
        source::{app_icon::DesktopEntries, niri::Placement},
    },
    message_subscriptions::niri_action,
    state::{self, PanelLayout, ToastPreferences},
    wayland_layers::panel_regions::Zone,
    widget_groups::notification_group::NotificationToggle,
    widgets::{reading_alert::ReadingAlert, reveal::reveal_y},
};

const DEFAULT_ACTION: &str = "default";

const CONFIG_ERROR_TOAST_ID: u32 = 0;

/// How long the compositor gets to answer `get_activation_token` before the
/// action is invoked without one.
const ACTIVATION_TOKEN_GRACE: Duration = Duration::from_millis(300);

/// Keys vetrata resolves itself against a niri window, rather than handing back
/// to the sender.
fn is_window_action(key: &str) -> bool {
    matches!(key, "move" | "quit")
}

pub fn view<'a>(bar: &'a Bar, window: Id, output: &str) -> Element<'a, Message> {
    bar.notifications.view(
        window,
        output,
        &bar.config,
        bar.theme.extended_palette(),
        &bar.panel_settings.layout,
        bar.bar_clearance(output),
    )
}

#[derive(Default)]
pub struct NotificationLayer {
    handle: Option<NotifHandle>,
    sound_theme: Option<String>,
    live: Vec<Notification>,
    next_seq: u64,
    hovered: Option<u32>,
    dismissed_reading_alerts: Vec<DismissedReadingAlert>,
}

impl NotificationLayer {
    pub fn adopt_sound_theme(&mut self, theme: Option<String>) {
        self.sound_theme = theme;
    }

    pub fn update(
        &mut self,
        message: NotificationMessage,
        placement: &Placement,
        entries: Option<&DesktopEntries>,
        config: &Config,
        toast_preferences: &ToastPreferences,
        panel_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Message> {
        match message {
            NotificationMessage::Connected(handle) => {
                self.handle = Some(handle);
                Task::none()
            }
            NotificationMessage::Served(data) => self.serve(
                data,
                ToastOrigin::DbusClient,
                placement,
                entries,
                config,
                toast_preferences,
            ),
            NotificationMessage::BodyClicked(id) => {
                self.activate_action(id, DEFAULT_ACTION, placement, panel_on)
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
                self.close_all([id], close_reason::DISMISSED)
            }
            NotificationMessage::ActionClicked { id, key } => {
                self.activate_action(id, &key, placement, panel_on)
            }
            NotificationMessage::DefaultActivated { id, token } => self.invoke_default(id, token),
            NotificationMessage::CloseRequested(id) => {
                self.close_all([id], close_reason::CLOSED_BY_CALL)
            }
            NotificationMessage::Rects { window, rects } => Task::done(Message::PillBounds {
                window,
                zone: Zone::Toasts,
                rects,
            }),
            NotificationMessage::Tick => self.expire(),
        }
    }

    pub fn show_reading_alerts(
        &mut self,
        active_alerts: &[ReadingAlert<Message>],
        niri: &EventStreamState,
        entries: Option<&DesktopEntries>,
        config: &Config,
        toast_preferences: &ToastPreferences,
    ) -> Task<Message> {
        if toast_preferences.do_not_disturb {
            return Task::none();
        }
        let changes = self.plan_reading_alert_toasts(active_alerts);
        if changes.is_empty() {
            return Task::none();
        }
        let placement = Placement::derive(niri);
        self.apply_reading_alert_toast_changes(
            changes,
            &placement,
            entries,
            config,
            toast_preferences,
        )
    }

    pub fn show_config_error(
        &mut self,
        path: &Path,
        error: &LoadError,
        placement: &Placement,
        entries: Option<&DesktopEntries>,
        config: &Config,
        toast_preferences: &ToastPreferences,
    ) -> Task<Message> {
        let data = NotificationData {
            id: CONFIG_ERROR_TOAST_ID,
            app_name: "vetrata".to_owned(),
            summary: format!("Invalid configuration in {}", path.display()),
            body: error.to_string(),
            severity: Severity::Critical,
            expire_timeout: ExpireTimeout::Never,
            ..NotificationData::default()
        };
        self.serve(
            Box::new(data),
            ToastOrigin::ConfigError,
            placement,
            entries,
            config,
            toast_preferences,
        )
    }

    pub fn withdraw_config_error(&mut self) -> Task<Message> {
        self.close_all([CONFIG_ERROR_TOAST_ID], close_reason::CLOSED_BY_CALL)
    }

    fn plan_reading_alert_toasts(
        &self,
        active_alerts: &[ReadingAlert<Message>],
    ) -> ReadingAlertToastChanges<Message> {
        let shown_toasts: Vec<ShownReadingAlertToast<'_>> = self
            .showing()
            .filter_map(|n| {
                n.reading_alert().map(|alert| ShownReadingAlertToast {
                    toast_id: n.data.id,
                    hover_target: &alert.hover_target,
                    severity: alert.severity,
                })
            })
            .collect();
        plan_reading_alert_toast_changes(
            active_alerts,
            &shown_toasts,
            &self.dismissed_reading_alerts,
        )
    }

    fn apply_reading_alert_toast_changes(
        &mut self,
        changes: ReadingAlertToastChanges<Message>,
        placement: &Placement,
        entries: Option<&DesktopEntries>,
        config: &Config,
        toast_preferences: &ToastPreferences,
    ) -> Task<Message> {
        self.dismissed_reading_alerts.retain(|dismissed| {
            !changes
                .dismissals_to_forget
                .contains(&dismissed.hover_target)
        });
        let mut tasks =
            vec![self.close_all(changes.toast_ids_to_close, close_reason::CLOSED_BY_CALL)];
        tasks.extend(changes.alerts_to_show.into_iter().map(|to_show| {
            let toast_id = match to_show.toast_slot {
                ToastSlotForAlert::ReplaceShownToast(toast_id) => toast_id,
                ToastSlotForAlert::OpenNewToast => self.fresh_reading_alert_toast_id(),
            };
            self.serve(
                Box::new(to_show.alert.toast_notification_data(toast_id)),
                ToastOrigin::ReadingAlert(Box::new(to_show.alert)),
                placement,
                entries,
                config,
                toast_preferences,
            )
        }));
        Task::batch(tasks)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn fresh_reading_alert_toast_id(&self) -> u32 {
        u32::MAX.wrapping_sub(self.next_seq as u32)
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

    fn reading_alert_message_for(&self, toast_id: u32, action_key: &str) -> Option<Message> {
        let alert = self
            .showing()
            .find(|n| n.data.id == toast_id)?
            .reading_alert()?;
        if action_key == DEFAULT_ACTION {
            return Some(alert.on_body_clicked.clone());
        }
        alert.action_button_pressed(action_key).cloned()
    }

    fn serve(
        &mut self,
        data: Box<NotificationData>,
        origin: ToastOrigin,
        placement: &Placement,
        entries: Option<&DesktopEntries>,
        config: &Config,
        toast_preferences: &ToastPreferences,
    ) -> Task<Message> {
        if toast_preferences.do_not_disturb {
            return Task::none();
        }
        let id = data.id;
        self.live.retain(|n| n.data.id != id);
        let mut tasks = Vec::new();
        if let Some(window) = data.window_id_hint {
            let dupes: Vec<u32> = self
                .showing()
                .filter(|n| n.data.window_id_hint == Some(window))
                .map(|n| n.data.id)
                .collect();
            tasks.push(self.close_all(dupes, close_reason::DISMISSED));
        }
        if toast_preferences.sound && !data.suppress_sound {
            tasks.push(sound::play(&data, self.sound_theme.as_deref()));
        }
        let icons = toast_card::resolve_icons(&data, entries);
        let seq = self.next_seq;
        self.next_seq += 1;
        let now = Instant::now();
        let countdown =
            expiry_countdown::lifetime(data.expire_timeout, data.severity, &config.toasts.lifetime)
                .map(|remaining| Countdown::started(remaining, now));
        let place = placement_for(&data, placement, toast_preferences, config);
        self.live.push(Notification::appearing(
            *data, origin, icons, countdown, seq, place, now,
        ));
        self.sync_countdowns(now);
        Task::batch(tasks)
    }

    pub fn on_niri_changed(
        &mut self,
        placement: &Placement,
        config: &Config,
        toast_preferences: &ToastPreferences,
    ) -> Task<Message> {
        let stale: Vec<u32> = self
            .showing()
            .filter(|n| is_stale(&n.data, placement))
            .map(|n| n.data.id)
            .collect();
        let closed = self.close_all(stale, close_reason::UNDEFINED);
        self.reposition(placement, config, toast_preferences);
        closed
    }

    pub fn prune(&mut self) {
        let now = Instant::now();
        self.live.retain(|n| !n.has_faded(now));
    }

    fn expire(&mut self) -> Task<Message> {
        let now = Instant::now();
        let expired: Vec<u32> = self
            .showing()
            .filter(|n| n.countdown.is_some_and(|c| c.has_elapsed(now)))
            .map(|n| n.data.id)
            .collect();
        let closed = self.close_all(expired, close_reason::EXPIRED);
        self.prune();
        closed
    }

    fn showing(&self) -> impl Iterator<Item = &Notification> {
        self.live.iter().filter(|n| !n.is_withdrawing())
    }

    fn sync_countdowns(&mut self, now: Instant) {
        let hovered = self.hovered;
        self.live.iter_mut().for_each(|notif| {
            let under_pointer = hovered == Some(notif.data.id);
            notif.countdown = notif.countdown.map(|countdown| {
                if under_pointer {
                    countdown.paused(now)
                } else {
                    countdown.resumed(now)
                }
            });
        });
    }

    /// Starts the fade-out and reports which notifications were actually
    /// showing, so the caller can order their `Closed` signals against
    /// whatever else it is emitting.
    fn withdraw_all(&mut self, candidates: impl IntoIterator<Item = u32>) -> Vec<u32> {
        let now = Instant::now();
        let closing: Vec<u32> = candidates
            .into_iter()
            .filter(|id| self.showing().any(|n| n.data.id == *id))
            .collect();
        self.live
            .iter_mut()
            .filter(|n| closing.contains(&n.data.id))
            .for_each(|n| n.withdraw(now));
        self.prune_hover();
        closing
    }

    fn close_all(
        &mut self,
        candidates: impl IntoIterator<Item = u32>,
        reason: u32,
    ) -> Task<Message> {
        let closed = self
            .withdraw_all(candidates)
            .into_iter()
            .filter(|id| self.sent_by_dbus_client(*id))
            .map(|id| Outbound::Closed { id, reason })
            .collect();
        self.emit(closed)
    }

    fn sent_by_dbus_client(&self, id: u32) -> bool {
        self.live
            .iter()
            .any(|n| n.data.id == id && n.sent_by_dbus_client())
    }

    fn prune_hover(&mut self) {
        let hovered = self.hovered;
        self.hovered = hovered.filter(|id| self.showing().any(|n| n.data.id == *id));
    }

    pub fn has_running_countdown(&self) -> bool {
        self.live
            .iter()
            .any(|n| n.countdown.is_some_and(Countdown::is_running))
    }

    fn window_of(&self, id: u32, placement: &Placement) -> Option<u64> {
        let notif = self.showing().find(|n| n.data.id == id)?;
        resolve_window_id(&notif.data, placement)
            .filter(|&w| placement.workspace_of_window(w).is_some())
    }

    fn declares_default_action(&self, id: u32) -> bool {
        self.showing()
            .any(|n| n.data.id == id && n.data.actions.iter().any(|a| a.key == DEFAULT_ACTION))
    }

    fn activate_action(
        &mut self,
        id: u32,
        key: &str,
        placement: &Placement,
        panel_on: &dyn Fn(&str) -> Option<Id>,
    ) -> Task<Message> {
        if let Some(reading_alert_message) = self.reading_alert_message_for(id, key) {
            self.remember_dismissed_reading_alert(id);
            return Task::batch([
                Task::done(reading_alert_message),
                self.close_all([id], close_reason::DISMISSED),
            ]);
        }
        if key == DEFAULT_ACTION && self.declares_default_action(id) {
            return self.request_activation_token(id, panel_on);
        }

        let survives_action_invocation = self.showing().any(|n| n.data.id == id && n.data.resident);
        let window = self.window_of(id, placement);
        let activation = match (key, window) {
            (DEFAULT_ACTION, Some(window)) => {
                niri_action(niri_ipc::Action::FocusWindow { id: window })
            }
            (DEFAULT_ACTION, None) => Task::none(),
            ("quit", Some(window)) => {
                niri_action(niri_ipc::Action::CloseWindow { id: Some(window) })
            }
            ("move", Some(window)) => {
                placement
                    .active_workspace_for_move()
                    .map_or_else(Task::none, |target| {
                        niri_action(niri_ipc::Action::MoveWindowToWorkspace {
                            window_id: Some(window),
                            reference: niri_ipc::WorkspaceReferenceArg::Id(target),
                            focus: true,
                        })
                    })
            }
            _ => self.emit(vec![Outbound::ActionInvoked {
                id,
                key: key.to_owned(),
            }]),
        };

        if survives_action_invocation {
            activation
        } else {
            Task::batch([activation, self.close_all([id], close_reason::DISMISSED)])
        }
    }

    /// A notification that ships its own default action knows which of its
    /// windows the user meant; vetrata cannot. So the click is handed back with
    /// an activation token and the sender raises itself.
    fn request_activation_token(
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

    fn invoke_default(&mut self, id: u32, token: Option<String>) -> Task<Message> {
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
            return self.emit(outbound);
        }

        outbound.extend(
            self.withdraw_all([id])
                .into_iter()
                .map(|id| Outbound::Closed {
                    id,
                    reason: close_reason::DISMISSED,
                }),
        );
        self.emit(outbound)
    }

    fn view<'a>(
        &'a self,
        window: Id,
        output: &str,
        config: &'a Config,
        palette: &'a Extended,
        layout: &'a PanelLayout,
        panel_clearance: Padding,
    ) -> Element<'a, Message> {
        let now = Instant::now();
        let mut ordered: Vec<&Notification> = self
            .live
            .iter()
            .filter(|n| n.place.output.as_deref() == Some(output))
            .collect();
        ordered.sort_by_key(|n| Reverse((n.place.anchor.is_window_bound(), n.seq)));
        let slots: Vec<ToastSlot<'_>> = ordered
            .into_iter()
            .map(|notif| {
                let reveal = notif.reveal(now);
                ToastSlot {
                    id: notif.data.id,
                    anchor: notif.place.anchor,
                    card: reveal_y(
                        toast_card::Card::new(notif, config, palette, layout)
                            .hovered(self.hovered == Some(notif.data.id))
                            .reveal(reveal),
                        reveal,
                    ),
                    animating: notif.is_animating(now),
                }
            })
            .collect();
        toast_field::field(window, slots, panel_clearance, &config.toasts)
            .map(Message::Notification)
    }

    pub fn is_active(&self) -> bool {
        !self.live.is_empty()
    }

    pub fn is_transitioning(&self) -> bool {
        let now = Instant::now();
        self.live
            .iter()
            .any(|n| n.is_animating(now) || n.is_withdrawing())
    }

    pub fn live_count(&self) -> usize {
        self.live.len()
    }

    pub fn toggle(
        &mut self,
        toggle: NotificationToggle,
        toast_preferences: &mut ToastPreferences,
        placement: &Placement,
        config: &Config,
    ) -> Task<Message> {
        match toggle {
            NotificationToggle::DoNotDisturb => {
                toast_preferences.do_not_disturb = !toast_preferences.do_not_disturb;
                self.sync_dnd(toast_preferences);
                Task::future(state::panel::save_do_not_disturb(
                    toast_preferences.do_not_disturb,
                ))
                .discard()
            }
            NotificationToggle::Sound => {
                toast_preferences.sound = !toast_preferences.sound;
                Task::future(state::panel::save_sound(toast_preferences.sound)).discard()
            }
            NotificationToggle::SpatialLayout => {
                toast_preferences.spatial_layout = !toast_preferences.spatial_layout;
                Task::batch([
                    self.on_niri_changed(placement, config, toast_preferences),
                    Task::future(state::panel::save_spatial_layout(
                        toast_preferences.spatial_layout,
                    ))
                    .discard(),
                ])
            }
        }
    }

    fn sync_dnd(&mut self, toast_preferences: &ToastPreferences) {
        if toast_preferences.do_not_disturb {
            self.live.clear();
            self.hovered = None;
        }
    }

    /// Signals reach the bus in the order given: a client reading an
    /// `ActivationToken` only associates it with the action that follows it.
    fn emit(&self, outbound: Vec<Outbound>) -> Task<Message> {
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

    fn reposition(
        &mut self,
        placement: &Placement,
        config: &Config,
        toast_preferences: &ToastPreferences,
    ) {
        self.live.iter_mut().for_each(|notif| {
            notif.place = placement_for(&notif.data, placement, toast_preferences, config);
        });
    }
}

fn is_stale(data: &NotificationData, placement: &Placement) -> bool {
    let Some(window) = resolve_window_id(data, placement) else {
        return false;
    };
    let window_owned = data.actions.iter().all(|a| is_window_action(&a.key));
    window_owned && placement.workspace_of_window(window).is_none()
}
