pub mod body_markup;
pub mod expiry_countdown;
pub mod output_surfaces;
pub mod placement_policy;
pub mod toast_anchor;
pub mod toast_card;
pub mod toast_field;
pub mod toast_lifecycle;

use std::{cmp::Reverse, collections::HashSet};

use expiry_countdown::Countdown;
use iced::{
    Element, Task,
    time::{Duration, Instant},
    window::Id,
};
use iced_exwlshell::{actions::ActionCallback, reexport::ActivationTokenSink};
use output_surfaces::ToastSurfaces;
use placement_policy::{placement_for, resolve_window_id};
use toast_anchor::ToastSlot;
pub use toast_lifecycle::{Notification, NotificationMessage};
use tokio::{sync::mpsc, time::timeout};

use crate::{
    app::Message,
    config::Config,
    data::{
        notifications::model::{NotifHandle, NotificationData, Outbound, close_reason},
        sink::sound,
        source::{app_icon::DesktopEntries, niri::Placement},
    },
    message_subscriptions::niri_action,
    state::{self, NotificationState},
    widget_groups::notification_group::NotificationToggle,
    widgets::reveal::reveal_y,
};

const DEFAULT_ACTION: &str = "default";

/// How long the compositor gets to answer `get_activation_token` before the
/// action is invoked without one.
const ACTIVATION_TOKEN_GRACE: Duration = Duration::from_millis(300);

/// Keys vetrata resolves itself against a niri window, rather than handing back
/// to the sender.
fn is_window_action(key: &str) -> bool {
    matches!(key, "move" | "quit")
}

#[derive(Default)]
pub struct NotificationLayer {
    handle: Option<NotifHandle>,
    sound_theme: Option<String>,
    live: Vec<Notification>,
    surfaces: ToastSurfaces,
    next_seq: u64,
    hovered: Option<u32>,
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
        notif_state: &NotificationState,
    ) -> Task<Message> {
        match message {
            NotificationMessage::Connected(handle) => {
                self.handle = Some(handle);
                Task::none()
            }
            NotificationMessage::Served(data) => {
                self.serve(data, placement, entries, config, notif_state)
            }
            NotificationMessage::BodyClicked(id) => {
                self.activate_action(id, DEFAULT_ACTION, placement, config, notif_state)
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
                self.dismiss(id, close_reason::DISMISSED, placement, config, notif_state)
            }
            NotificationMessage::ActionClicked { id, key } => {
                self.activate_action(id, &key, placement, config, notif_state)
            }
            NotificationMessage::DefaultActivated { id, token } => {
                self.invoke_default(id, token, placement, config, notif_state)
            }
            NotificationMessage::CloseRequested(id) => self.dismiss(
                id,
                close_reason::CLOSED_BY_CALL,
                placement,
                config,
                notif_state,
            ),
            NotificationMessage::Rects { window, rects } => Task::done(Message::SetInputRegion {
                id: window,
                callback: ActionCallback::new(move |region| {
                    rects.iter().for_each(|rect| {
                        region.add(
                            rect.x as i32,
                            rect.y as i32,
                            rect.width as i32,
                            rect.height as i32,
                        );
                    });
                }),
            }),
            NotificationMessage::Tick => self.expire(placement, config, notif_state),
        }
    }

    fn serve(
        &mut self,
        data: Box<NotificationData>,
        placement: &Placement,
        entries: Option<&DesktopEntries>,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        if notif_state.dnd {
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
        if notif_state.sound && !data.suppress_sound {
            tasks.push(sound::play(&data, self.sound_theme.as_deref()));
        }
        let icons = toast_card::resolve_icons(&data, entries);
        let seq = self.next_seq;
        self.next_seq += 1;
        let now = Instant::now();
        let countdown =
            expiry_countdown::lifetime(data.expire_timeout, data.urgency, &config.notifications)
                .map(|remaining| Countdown::started(remaining, now));
        let place = placement_for(&data, placement, notif_state, config);
        self.live.push(Notification::appearing(
            *data, icons, countdown, seq, place, now,
        ));
        self.sync_countdowns(now);
        tasks.push(self.reconcile(placement, config, notif_state));
        Task::batch(tasks)
    }

    pub fn on_niri_changed(
        &mut self,
        placement: &Placement,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        let stale: Vec<u32> = self
            .showing()
            .filter(|n| is_stale(&n.data, placement))
            .map(|n| n.data.id)
            .collect();
        let mut tasks = vec![self.close_all(stale, close_reason::UNDEFINED)];
        tasks.push(self.reconcile(placement, config, notif_state));
        Task::batch(tasks)
    }

    pub fn prune(
        &mut self,
        placement: &Placement,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        let now = Instant::now();
        let before = self.live.len();
        self.live.retain(|n| !n.has_faded(now));
        if self.live.len() == before {
            return Task::none();
        }
        self.reconcile(placement, config, notif_state)
    }

    fn expire(
        &mut self,
        placement: &Placement,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        let now = Instant::now();
        let expired: Vec<u32> = self
            .showing()
            .filter(|n| n.countdown.is_some_and(|c| c.has_elapsed(now)))
            .map(|n| n.data.id)
            .collect();
        let mut tasks = vec![self.close_all(expired, close_reason::EXPIRED)];
        tasks.push(self.prune(placement, config, notif_state));
        Task::batch(tasks)
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
            .map(|id| Outbound::Closed { id, reason })
            .collect();
        self.emit(closed)
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
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        if key == DEFAULT_ACTION && self.declares_default_action(id) {
            return self.request_activation_token(id);
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
            Task::batch([
                activation,
                self.dismiss(id, close_reason::DISMISSED, placement, config, notif_state),
            ])
        }
    }

    /// A notification that ships its own default action knows which of its
    /// windows the user meant; vetrata cannot. So the click is handed back with
    /// an activation token and the sender raises itself.
    fn request_activation_token(&self, id: u32) -> Task<Message> {
        let toast_surface = self
            .showing()
            .find(|n| n.data.id == id)
            .and_then(|n| self.surfaces.window_on(n.place.output.as_ref()));
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

    fn invoke_default(
        &mut self,
        id: u32,
        token: Option<String>,
        placement: &Placement,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
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
        Task::batch([
            self.emit(outbound),
            self.reconcile(placement, config, notif_state),
        ])
    }

    pub fn view(&self, window: Id, config: &Config) -> Option<Element<'_, Message>> {
        let output = self.surfaces.output_of(window)?;
        let now = Instant::now();
        let mut ordered: Vec<&Notification> = self
            .live
            .iter()
            .filter(|n| n.place.output == *output)
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
                        toast_card::Card::new(notif, config)
                            .hovered(self.hovered == Some(notif.data.id))
                            .reveal(reveal),
                        reveal,
                    ),
                    animating: notif.is_animating(now),
                }
            })
            .collect();
        Some(toast_field::field(window, slots, &config.notifications).map(Message::Notification))
    }

    pub fn forget(&mut self, window: Id) {
        self.surfaces.forget(window);
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
        notif_state: &mut NotificationState,
        placement: &Placement,
        config: &Config,
    ) -> Task<Message> {
        match toggle {
            NotificationToggle::DoNotDisturb => {
                notif_state.dnd = !notif_state.dnd;
                Task::batch([
                    self.sync_dnd(placement, config, notif_state),
                    Task::future(state::panel::save_dnd(notif_state.dnd)).discard(),
                ])
            }
            NotificationToggle::Sound => {
                notif_state.sound = !notif_state.sound;
                Task::future(state::panel::save_sound(notif_state.sound)).discard()
            }
            NotificationToggle::SpatialLayout => {
                notif_state.spatial_layout = !notif_state.spatial_layout;
                Task::batch([
                    self.on_niri_changed(placement, config, notif_state),
                    Task::future(state::panel::save_spatial_layout(
                        notif_state.spatial_layout,
                    ))
                    .discard(),
                ])
            }
        }
    }

    pub fn sync_dnd(
        &mut self,
        placement: &Placement,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        if notif_state.dnd {
            self.live.clear();
            self.hovered = None;
        }
        self.reconcile(placement, config, notif_state)
    }

    fn dismiss(
        &mut self,
        id: u32,
        reason: u32,
        placement: &Placement,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        Task::batch([
            self.close_all([id], reason),
            self.reconcile(placement, config, notif_state),
        ])
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

    fn reconcile(
        &mut self,
        placement: &Placement,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        self.live.iter_mut().for_each(|notif| {
            notif.place = placement_for(&notif.data, placement, notif_state, config);
        });
        let desired: HashSet<Option<String>> = if notif_state.dnd {
            HashSet::new()
        } else {
            self.live
                .iter()
                .map(|notif| notif.place.output.clone())
                .collect()
        };
        self.surfaces.reconcile(&desired)
    }
}

fn is_stale(data: &NotificationData, placement: &Placement) -> bool {
    let Some(window) = resolve_window_id(data, placement) else {
        return false;
    };
    let window_owned = data.actions.iter().all(|a| is_window_action(&a.key));
    window_owned && placement.workspace_of_window(window).is_none()
}
