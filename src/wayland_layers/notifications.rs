pub mod card;
pub mod countdown;
pub mod markup;
pub mod placement;
pub mod stack;

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    time::{Instant, SystemTime},
};

use countdown::Countdown;
use iced::{Element, Task, widget::Sensor, window::Id};
use iced_layershell::reexport::Anchor;
use placement::{ToastPlacement, placement_for, resolve_window_id};

use crate::{
    app::Message,
    config::resolved::ResolvedConfig,
    data::{
        notifications::model::{NotifHandle, NotificationData, Outbound, close_reason},
        sink::sound,
        source::{
            app_icon::DesktopEntries,
            niri::{self, HEdge, OutputGeometry, Placement, VEdge},
        },
    },
    message_subscriptions::niri_action,
    wayland_layers::surface::{Margin, SurfaceSize},
};

fn is_window_action(key: &str) -> bool {
    matches!(key, "default" | "move" | "quit")
}

#[derive(Debug, Clone)]
pub enum NotificationMessage {
    Connected(NotifHandle),
    Served(Box<NotificationData>),
    CloseRequested(u32),
    BodyClicked(u32),
    HoverChanged { id: u32, hovered: bool },
    Dismiss(u32),
    ActionClicked { id: u32, key: String },
    Resized { id: u32, height: f32 },
    Tick,
}

pub struct Notification {
    pub data: NotificationData,
    pub created_at: SystemTime,
    countdown: Option<Countdown>,
    seq: u64,
    pub icons: card::CardIcons,
    measured_height: f32,
}

struct ToastSurface {
    id: Id,
    output: Option<String>,
    anchor: Anchor,
    margin: Margin,
    size: SurfaceSize,
}

#[derive(Default)]
pub struct NotificationLayer {
    handle: Option<NotifHandle>,
    live: Vec<Notification>,
    surfaces: HashMap<u32, ToastSurface>,
    next_seq: u64,
    hovered: Option<u32>,
}

impl NotificationLayer {
    pub fn update(
        &mut self,
        message: NotificationMessage,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        entries: Option<&DesktopEntries>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        match message {
            NotificationMessage::Connected(handle) => {
                self.handle = Some(handle);
                Task::none()
            }
            NotificationMessage::Served(data) => {
                if config.dnd {
                    return Task::none();
                }
                let id = data.id;
                self.live.retain(|n| n.data.id != id);
                let mut tasks = Vec::new();
                if let Some(window) = data.window_id_hint {
                    let dupes: Vec<u32> = self
                        .live
                        .iter()
                        .filter(|n| n.data.window_id_hint == Some(window))
                        .map(|n| n.data.id)
                        .collect();
                    for dup in dupes {
                        self.live.retain(|n| n.data.id != dup);
                        tasks.push(self.emit(Outbound::Closed {
                            id: dup,
                            reason: close_reason::DISMISSED,
                        }));
                    }
                    self.prune_hover();
                }
                if config.sound && !data.suppress_sound {
                    tasks.push(sound::play(&data, config.sound_theme.as_deref()));
                }
                let icons = card::resolve_icons(&data, entries);
                let measured_height = card::measure_height(&data, &icons, config);
                let seq = self.next_seq;
                self.next_seq += 1;
                let now = Instant::now();
                let countdown =
                    countdown::lifetime(data.expire_timeout, data.urgency, &config.notifications)
                        .map(|remaining| Countdown::started(remaining, now));
                self.live.push(Notification {
                    data: *data,
                    created_at: SystemTime::now(),
                    countdown,
                    seq,
                    icons,
                    measured_height,
                });
                self.sync_countdowns(now);
                tasks.push(self.reconcile(placement, outputs, config));
                Task::batch(tasks)
            }
            NotificationMessage::BodyClicked(id) => match self.window_of(id, placement) {
                Some(window) => Task::batch([
                    niri_action(move || niri::focus_window(window)),
                    self.dismiss(id, close_reason::DISMISSED, placement, outputs, config),
                ]),
                None => self.dismiss(id, close_reason::DISMISSED, placement, outputs, config),
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
                self.dismiss(id, close_reason::DISMISSED, placement, outputs, config)
            }
            NotificationMessage::ActionClicked { id, key } => {
                self.activate_action(id, &key, placement, outputs, config)
            }
            NotificationMessage::CloseRequested(id) => {
                self.dismiss(id, close_reason::CLOSED_BY_CALL, placement, outputs, config)
            }
            NotificationMessage::Resized { id, height } => {
                self.adopt_measured_height(id, height, placement, outputs, config)
            }
            NotificationMessage::Tick => self.expire(placement, outputs, config),
        }
    }

    pub fn on_niri_changed(
        &mut self,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        let stale: Vec<u32> = self
            .live
            .iter()
            .filter(|n| is_stale(&n.data, placement))
            .map(|n| n.data.id)
            .collect();
        let mut tasks = Vec::new();
        for id in stale {
            self.live.retain(|n| n.data.id != id);
            tasks.push(self.emit(Outbound::Closed {
                id,
                reason: close_reason::UNDEFINED,
            }));
        }
        self.prune_hover();
        tasks.push(self.reconcile(placement, outputs, config));
        Task::batch(tasks)
    }

    fn expire(
        &mut self,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        let now = Instant::now();
        let expired: Vec<u32> = self
            .live
            .iter()
            .filter(|n| n.countdown.is_some_and(|c| c.has_elapsed(now)))
            .map(|n| n.data.id)
            .collect();
        if expired.is_empty() {
            return Task::none();
        }
        self.live.retain(|n| !expired.contains(&n.data.id));
        self.prune_hover();
        let mut tasks: Vec<Task<Message>> = expired
            .into_iter()
            .map(|id| {
                self.emit(Outbound::Closed {
                    id,
                    reason: close_reason::EXPIRED,
                })
            })
            .collect();
        tasks.push(self.reconcile(placement, outputs, config));
        Task::batch(tasks)
    }

    fn sync_countdowns(&mut self, now: Instant) {
        let hovered = self.hovered;
        for notif in &mut self.live {
            let under_pointer = hovered == Some(notif.data.id);
            notif.countdown = notif.countdown.map(|countdown| {
                if under_pointer {
                    countdown.paused(now)
                } else {
                    countdown.resumed(now)
                }
            });
        }
    }

    fn prune_hover(&mut self) {
        let live = &self.live;
        self.hovered = self
            .hovered
            .filter(|id| live.iter().any(|n| n.data.id == *id));
    }

    pub fn has_running_countdown(&self) -> bool {
        self.live
            .iter()
            .any(|n| n.countdown.is_some_and(Countdown::is_running))
    }

    fn adopt_measured_height(
        &mut self,
        id: u32,
        height: f32,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        let Some(notif) = self.live.iter_mut().find(|n| n.data.id == id) else {
            return Task::none();
        };
        if stack::surface_height(height) == stack::surface_height(notif.measured_height) {
            return Task::none();
        }
        notif.measured_height = height;
        self.reconcile(placement, outputs, config)
    }

    fn window_of(&self, id: u32, placement: &Placement) -> Option<u64> {
        let notif = self.live.iter().find(|n| n.data.id == id)?;
        resolve_window_id(&notif.data, placement)
            .filter(|&w| placement.workspace_of_window(w).is_some())
    }

    fn activate_action(
        &mut self,
        id: u32,
        key: &str,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        let survives_action_invocation =
            self.live.iter().any(|n| n.data.id == id && n.data.resident);
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
                self.dismiss(id, close_reason::DISMISSED, placement, outputs, config),
            ])
        }
    }

    pub fn view(&self, id: Id, config: &ResolvedConfig) -> Option<Element<'_, Message>> {
        let (&notif_id, _) = self.surfaces.iter().find(|(_, s)| s.id == id)?;
        let notif = self.live.iter().find(|n| n.data.id == notif_id)?;
        let hovered = self.hovered == Some(notif_id);
        Some(
            Element::from(
                Sensor::new(card::card(notif, hovered, config)).on_resize(move |size| {
                    NotificationMessage::Resized {
                        id: notif_id,
                        height: size.height,
                    }
                }),
            )
            .map(Message::Notification),
        )
    }

    pub fn forget(&mut self, id: Id) {
        self.surfaces.retain(|_, s| s.id != id);
    }

    pub fn is_active(&self) -> bool {
        !self.live.is_empty()
    }

    pub fn sync_dnd(
        &mut self,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        if config.dnd {
            self.live.clear();
            self.hovered = None;
        }
        self.reconcile(placement, outputs, config)
    }

    fn dismiss(
        &mut self,
        id: u32,
        reason: u32,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        let existed = self.live.iter().any(|n| n.data.id == id);
        self.live.retain(|n| n.data.id != id);
        self.prune_hover();
        let closed = if existed {
            self.emit(Outbound::Closed { id, reason })
        } else {
            Task::none()
        };
        Task::batch([closed, self.reconcile(placement, outputs, config)])
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
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        let desired = self.desired_surfaces(placement, outputs, config);
        let changes = self.plan_changes(&desired);
        self.apply_changes(changes)
    }

    fn plan_changes(&self, desired: &[DesiredSurface]) -> Vec<SurfaceChange> {
        let desired_ids: HashSet<u32> = desired.iter().map(|wanted| wanted.id).collect();
        let opens_and_adjusts =
            desired
                .iter()
                .flat_map(|wanted| match self.surfaces.get(&wanted.id) {
                    None => vec![SurfaceChange::Open(wanted.clone())],
                    Some(existing) if existing.output != wanted.output => {
                        vec![
                            SurfaceChange::Close(wanted.id),
                            SurfaceChange::Open(wanted.clone()),
                        ]
                    }
                    Some(existing) => {
                        let anchor = (existing.anchor != wanted.anchor).then_some(wanted.anchor);
                        let margin = (existing.margin != wanted.margin).then_some(wanted.margin);
                        let size = (existing.size != wanted.size).then_some(wanted.size);
                        if anchor.is_none() && margin.is_none() && size.is_none() {
                            vec![]
                        } else {
                            vec![SurfaceChange::Adjust {
                                id: wanted.id,
                                anchor,
                                margin,
                                size,
                            }]
                        }
                    }
                });
        let closes = self
            .surfaces
            .keys()
            .copied()
            .filter(|id| !desired_ids.contains(id))
            .map(SurfaceChange::Close);
        opens_and_adjusts.chain(closes).collect()
    }

    fn apply_changes(&mut self, changes: Vec<SurfaceChange>) -> Task<Message> {
        Task::batch(
            changes
                .into_iter()
                .map(|change| self.perform_change(change)),
        )
    }

    fn perform_change(&mut self, change: SurfaceChange) -> Task<Message> {
        match change {
            SurfaceChange::Open(wanted) => {
                let window = Id::unique();
                let settings = stack::toast_settings(
                    wanted.output.as_deref(),
                    wanted.anchor,
                    wanted.margin,
                    wanted.size,
                );
                self.surfaces.insert(
                    wanted.id,
                    ToastSurface {
                        id: window,
                        output: wanted.output,
                        anchor: wanted.anchor,
                        margin: wanted.margin,
                        size: wanted.size,
                    },
                );
                Task::done(Message::NewLayerShell {
                    settings,
                    id: window,
                })
            }
            SurfaceChange::Adjust {
                id,
                anchor,
                margin,
                size,
            } => {
                let Some(existing) = self.surfaces.get_mut(&id) else {
                    return Task::none();
                };
                let window = existing.id;
                let mut tasks = Vec::new();
                if let Some(anchor) = anchor {
                    existing.anchor = anchor;
                    tasks.push(Task::done(Message::AnchorChange { id: window, anchor }));
                }
                if let Some(margin) = margin {
                    existing.margin = margin;
                    tasks.push(Task::done(Message::MarginChange {
                        id: window,
                        margin: margin.edges(),
                    }));
                }
                if let Some(size) = size {
                    existing.size = size;
                    tasks.push(Task::done(Message::SizeChange {
                        id: window,
                        size: size.dimensions(),
                    }));
                }
                Task::batch(tasks)
            }
            SurfaceChange::Close(id) => match self.surfaces.remove(&id) {
                Some(existing) => Task::done(Message::RemoveWindow(existing.id)),
                None => Task::none(),
            },
        }
    }

    fn desired_surfaces(
        &self,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Vec<DesiredSurface> {
        if config.dnd {
            return Vec::new();
        }
        let width = config.notifications.card_width.round() as u32;
        let mut order: Vec<&Notification> = self.live.iter().collect();
        order.sort_by_key(|n| Reverse(n.seq));

        let placed: Vec<(&Notification, ToastPlacement, f32)> = order
            .iter()
            .map(|notif| {
                let place = placement_for(&notif.data, placement, config);
                (*notif, place, notif.measured_height)
            })
            .collect();

        let mut totals: HashMap<GroupKey, (f32, usize)> = HashMap::new();
        for (_, place, height) in &placed {
            let entry = totals.entry(group_key(place)).or_insert((0.0, 0));
            entry.0 += height;
            entry.1 += 1;
        }

        let mut offsets: HashMap<GroupKey, f32> = HashMap::new();
        placed
            .iter()
            .map(|(notif, place, height)| {
                let key = group_key(place);
                let (sum, count) = totals[&key];
                let group_height = sum + stack::STACK_SPACING * (count.saturating_sub(1)) as f32;
                let offset = offsets.entry(key).or_insert(0.0);
                let output_height = place
                    .output
                    .as_deref()
                    .and_then(|output| outputs.get(output))
                    .map(|geometry| geometry.height);
                let stack::SlotPlacement { anchor, margin } = stack::slot_placement(
                    place.vertical,
                    place.horizontal,
                    *offset,
                    group_height,
                    output_height,
                    config,
                );
                *offset += height + stack::STACK_SPACING;
                DesiredSurface {
                    id: notif.data.id,
                    output: place.output.clone(),
                    anchor,
                    margin,
                    size: SurfaceSize {
                        width,
                        height: stack::surface_height(notif.measured_height),
                    },
                }
            })
            .collect()
    }
}

type GroupKey = (Option<String>, VEdge, HEdge);

fn group_key(place: &ToastPlacement) -> GroupKey {
    (place.output.clone(), place.vertical, place.horizontal)
}

enum SurfaceChange {
    Open(DesiredSurface),
    Adjust {
        id: u32,
        anchor: Option<Anchor>,
        margin: Option<Margin>,
        size: Option<SurfaceSize>,
    },
    Close(u32),
}

#[derive(Clone)]
struct DesiredSurface {
    id: u32,
    output: Option<String>,
    anchor: Anchor,
    margin: Margin,
    size: SurfaceSize,
}

fn is_stale(data: &NotificationData, placement: &Placement) -> bool {
    let Some(window) = resolve_window_id(data, placement) else {
        return false;
    };
    let window_owned = data.actions.iter().all(|a| is_window_action(&a.key));
    window_owned && placement.workspace_of_window(window).is_none()
}
