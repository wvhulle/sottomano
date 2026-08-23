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
use iced::{Element, Task, time::Instant, window::Id};
use iced_layershell::actions::ActionCallback;
use output_surfaces::ToastSurfaces;
use placement_policy::{placement_for, resolve_window_id};
use toast_anchor::ToastSlot;
pub use toast_lifecycle::{Notification, NotificationMessage};

use crate::{
    app::Message,
    config::Config,
    data::{
        notifications::model::{NotifHandle, NotificationData, Outbound, close_reason},
        sink::sound,
        source::{app_icon::DesktopEntries, niri, niri::Placement},
    },
    message_subscriptions::niri_action,
    state::{self, NotificationState},
    widget_groups::notification_group::NotificationGroupMessage,
    widgets::reveal::reveal_y,
};

fn is_window_action(key: &str) -> bool {
    matches!(key, "default" | "move" | "quit")
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
            NotificationMessage::BodyClicked(id) => match self.window_of(id, placement) {
                Some(window) => Task::batch([
                    niri_action(move || niri::focus_window(window)),
                    self.dismiss(id, close_reason::DISMISSED, placement, config, notif_state),
                ]),
                None => self.dismiss(id, close_reason::DISMISSED, placement, config, notif_state),
            },
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
            tasks.extend(self.close_all(dupes, close_reason::DISMISSED));
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
        let mut tasks = self.close_all(stale, close_reason::UNDEFINED);
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
        let mut tasks = self.close_all(expired, close_reason::EXPIRED);
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

    fn close_all(
        &mut self,
        candidates: impl IntoIterator<Item = u32>,
        reason: u32,
    ) -> Vec<Task<Message>> {
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
            .into_iter()
            .map(|id| self.emit(Outbound::Closed { id, reason }))
            .collect()
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

    pub fn is_animating(&self) -> bool {
        let now = Instant::now();
        self.live.iter().any(|n| n.is_animating(now))
    }

    fn window_of(&self, id: u32, placement: &Placement) -> Option<u64> {
        let notif = self.showing().find(|n| n.data.id == id)?;
        resolve_window_id(&notif.data, placement)
            .filter(|&w| placement.workspace_of_window(w).is_some())
    }

    fn activate_action(
        &mut self,
        id: u32,
        key: &str,
        placement: &Placement,
        config: &Config,
        notif_state: &NotificationState,
    ) -> Task<Message> {
        let survives_action_invocation = self.showing().any(|n| n.data.id == id && n.data.resident);
        let activation = match self
            .window_of(id, placement)
            .filter(|_| is_window_action(key))
        {
            Some(window) => match key {
                "default" => niri_action(move || niri::focus_window(window)),
                "quit" => niri_action(move || niri::close_window(window)),
                "move" => placement
                    .active_workspace_for_move()
                    .map_or_else(Task::none, |target| {
                        niri_action(move || niri::move_window_here(window, target))
                    }),
                _ => Task::none(),
            },
            None => self.emit(Outbound::ActionInvoked {
                id,
                key: key.to_owned(),
            }),
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

    pub fn view(&self, window: Id, config: &Config) -> Option<Element<'_, Message>> {
        let output = self.surfaces.output_of(window)?;
        let now = Instant::now();
        let mut ordered: Vec<&Notification> = self
            .live
            .iter()
            .filter(|n| n.place.output == *output)
            .collect();
        ordered.sort_by_key(|n| Reverse(n.seq));
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

    pub fn toggle(
        &mut self,
        msg: NotificationGroupMessage,
        notif_state: &mut NotificationState,
        placement: &Placement,
        config: &Config,
    ) -> Task<Message> {
        match msg {
            NotificationGroupMessage::DoNotDisturb => {
                notif_state.dnd = !notif_state.dnd;
                Task::batch([
                    self.sync_dnd(placement, config, notif_state),
                    Task::future(state::panel::save_dnd(notif_state.dnd)).discard(),
                ])
            }
            NotificationGroupMessage::Sound => {
                notif_state.sound = !notif_state.sound;
                Task::future(state::panel::save_sound(notif_state.sound)).discard()
            }
            NotificationGroupMessage::SpatialLayout => {
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
        let mut tasks = self.close_all([id], reason);
        tasks.push(self.reconcile(placement, config, notif_state));
        Task::batch(tasks)
    }

    fn emit(&self, outbound: Outbound) -> Task<Message> {
        match &self.handle {
            Some(handle) => {
                let sender = handle.sender();
                Task::future(async move {
                    let _ = sender.send(outbound).await;
                })
                .discard()
            }
            None => Task::none(),
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
