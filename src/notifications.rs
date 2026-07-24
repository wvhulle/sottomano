//! The notification system: an `org.freedesktop.Notifications` server feeding
//! one layer-shell toast surface per live notification. vetrata owns the daemon
//! so a notification's action can jump to the niri window that sent it,
//! in-process, after the posting client dies.

pub mod card;
pub mod markup;
pub mod model;
pub mod placement;
pub mod server;
pub mod stack;

use std::{
    collections::{HashMap, HashSet},
    time::SystemTime,
};

use iced::{Element, Task, widget::Sensor, window::Id};
use iced_layershell::reexport::Anchor;

use crate::{
    bar_program::Message,
    config::resolved::ResolvedConfig,
    data_sources::{
        app_icon::DesktopEntries,
        niri::{self, HEdge, OutputGeometry, Placement, VEdge},
    },
    message_subscriptions::niri_action,
};
use model::{NotifHandle, NotificationData, Outbound, close_reason};
use placement::{ToastPlacement, placement_for, resolve_window_id};

/// Action keys odin runs itself against the hinted window instead of handing
/// back to the client: `default` (the freedesktop primary action) and the
/// `move`/`quit` verbs the window-notification producers use.
fn is_window_action(key: &str) -> bool {
    matches!(key, "default" | "move" | "quit")
}

#[derive(Debug, Clone)]
pub enum NotificationMessage {
    /// First message from the D-Bus bridge, carrying the handle the app emits
    /// signals through.
    Connected(NotifHandle),
    /// A client posted (or replaced) a notification.
    Served(NotificationData),
    /// A client called `CloseNotification`.
    CloseRequested(u32),
    /// A toast surface measured its card, so its surface can be sized to fit and
    /// its siblings re-stacked.
    CardMeasured { id: u32, height: f32 },
    /// The card body was left-clicked.
    BodyClicked(u32),
    /// The pointer entered or left a card, for its hover-border highlight.
    HoverChanged { id: u32, hovered: bool },
    /// The card was right-clicked, or otherwise dismissed by the user.
    Dismiss(u32),
    /// An action button was pressed.
    ActionClicked { id: u32, key: String },
}

/// One tracked notification. The freedesktop notification carries no timestamp,
/// so arrival is recorded here for the card's relative time. The app icon is
/// resolved once at arrival (off the render path), not per redraw.
pub struct Notification {
    pub data: NotificationData,
    pub created_at: SystemTime,
    pub icons: card::CardIcons,
}

/// The live layer-shell surface for one toast, and the geometry last applied to
/// it (so reconciliation emits only real changes).
struct ToastSurface {
    id: Id,
    output: Option<String>,
    anchor: Anchor,
    margin: (i32, i32, i32, i32),
    size: (u32, u32),
}

#[derive(Default)]
pub struct NotificationGroup {
    handle: Option<NotifHandle>,
    live: Vec<Notification>,
    surfaces: HashMap<u32, ToastSurface>,
    heights: HashMap<u32, f32>,
    /// The card the pointer is currently over, for its hover-border highlight.
    hovered: Option<u32>,
    dnd: bool,
}

impl NotificationGroup {
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
                let id = data.id;
                // A replacing notification (same id) supersedes the old one.
                self.live.retain(|n| n.data.id != id);
                // One live notification per window: an older toast from the same
                // window is answered or obsolete, and leaving it up is what
                // produces "I clicked it and nothing happened".
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
                        self.heights.remove(&dup);
                        tasks.push(self.emit(Outbound::Closed {
                            id: dup,
                            reason: close_reason::DISMISSED,
                        }));
                    }
                }
                let icons = card::resolve_icons(&data, entries);
                self.live.push(Notification {
                    data,
                    created_at: SystemTime::now(),
                    icons,
                });
                tasks.push(self.reconcile(placement, outputs, config));
                Task::batch(tasks)
            }
            NotificationMessage::CardMeasured { id, height } => {
                if self.heights.get(&id) == Some(&height) {
                    return Task::none();
                }
                self.heights.insert(id, height);
                self.reconcile(placement, outputs, config)
            }
            // Left-clicking the body invokes the primary action: jump to the
            // source window and dismiss when there is one, else just dismiss.
            NotificationMessage::BodyClicked(id) => {
                match self.window_of(id, placement) {
                    Some(window) => Task::batch([
                        niri_action(move || niri::focus_window(window)),
                        self.dismiss(id, close_reason::DISMISSED, placement, outputs, config),
                    ]),
                    None => {
                        self.dismiss(id, close_reason::DISMISSED, placement, outputs, config)
                    }
                }
            }
            // Hover changes only the border tint/width — a pure repaint, no
            // geometry change, so no reconcile. Guard the exit so a stray leave
            // from a card the pointer already left doesn't clear a newer hover.
            NotificationMessage::HoverChanged { id, hovered } => {
                if hovered {
                    self.hovered = Some(id);
                } else if self.hovered == Some(id) {
                    self.hovered = None;
                }
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
        }
    }

    /// After a niri change: dismiss notifications whose target window has closed
    /// (they can never serve their purpose), then recompute the surviving
    /// toasts' corners. Driven from `Bar::apply_niri_event`.
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
            self.heights.remove(&id);
            tasks.push(self.emit(Outbound::Closed {
                id,
                reason: close_reason::UNDEFINED,
            }));
        }
        tasks.push(self.reconcile(placement, outputs, config));
        Task::batch(tasks)
    }

    /// A notification's resolvable, still-live target window, if any.
    fn window_of(
        &self,
        id: u32,
        placement: &Placement,
    ) -> Option<u64> {
        let notif = self.live.iter().find(|n| n.data.id == id)?;
        resolve_window_id(&notif.data, placement)
            .filter(|&w| placement.workspace_of_window(w).is_some())
    }

    /// Run a clicked action. A recognised window action on a resolvable window is
    /// driven in niri in-process (works after the posting client has died);
    /// everything else round-trips over D-Bus as `ActionInvoked`, and — like a
    /// permission prompt's allow/deny — leaves the toast up for the client to
    /// close.
    fn activate_action(
        &mut self,
        id: u32,
        key: &str,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        if let Some(window) = self.window_of(id, placement).filter(|_| is_window_action(key))
        {
            let action = match key {
                "default" => niri_action(move || niri::focus_window(window)),
                "quit" => niri_action(move || niri::close_window(window)),
                "move" => match placement.active_workspace_for_move() {
                    Some(target) => niri_action(move || niri::move_window_here(window, target)),
                    None => Task::none(),
                },
                _ => Task::none(),
            };
            return Task::batch([
                action,
                self.dismiss(id, close_reason::DISMISSED, placement, outputs, config),
            ]);
        }
        self.emit(Outbound::ActionInvoked {
            id,
            key: key.to_owned(),
        })
    }

    /// The card view for toast surface `id`, wrapped in a size sensor that
    /// reports the card height back so the surface can shrink to fit and the
    /// stack can re-offset.
    pub fn view(&self, id: Id, config: &ResolvedConfig) -> Option<Element<'_, Message>> {
        let (&notif_id, _) = self.surfaces.iter().find(|(_, s)| s.id == id)?;
        let notif = self.live.iter().find(|n| n.data.id == notif_id)?;
        let hovered = self.hovered == Some(notif_id);
        let sensor = Sensor::new(card::card(notif, hovered, 1.0, config)).on_resize(move |size| {
            NotificationMessage::CardMeasured {
                id: notif_id,
                height: size.height,
            }
        });
        Some(Element::from(sensor).map(Message::Notification))
    }

    /// Drop the record of a toast surface the compositor already closed.
    pub fn forget(&mut self, id: Id) {
        self.surfaces.retain(|_, s| s.id != id);
    }

    /// Whether any toast is live — gates the per-niri-event placement recompute.
    pub fn is_active(&self) -> bool {
        !self.live.is_empty()
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
        self.heights.remove(&id);
        let closed = existed
            .then(|| self.emit(Outbound::Closed { id, reason }))
            .unwrap_or_else(Task::none);
        Task::batch([closed, self.reconcile(placement, outputs, config)])
    }

    /// Send an outbound signal to the D-Bus side, fire-and-forget.
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

    /// Bring the live layer-shell surfaces in line with the live notifications:
    /// a pure [`plan_changes`](Self::plan_changes) of the differences, then
    /// [`apply_changes`](Self::apply_changes) performs them — the decision is a
    /// value, and only the apply step touches the surface bookkeeping or emits
    /// commands.
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

    /// The surface changes that would bring the open set in line with `desired`.
    /// Pure — reads the current surfaces but mutates nothing and emits nothing,
    /// so it reads as a diff of two sets: open the new, adjust the moved, close
    /// the gone (or DND-suppressed).
    fn plan_changes(&self, desired: &[DesiredSurface]) -> Vec<SurfaceChange> {
        let desired_ids: HashSet<u32> = desired.iter().map(|wanted| wanted.id).collect();
        let opens_and_adjusts = desired
            .iter()
            .flat_map(|wanted| match self.surfaces.get(&wanted.id) {
                None => vec![SurfaceChange::Open(wanted.clone())],
                // A surface cannot move between outputs; close and reopen.
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

    /// Perform the planned changes: the sole place surfaces are opened, moved, or
    /// closed, and the sole place layer-shell commands are emitted.
    fn apply_changes(&mut self, changes: Vec<SurfaceChange>) -> Task<Message> {
        Task::batch(changes.into_iter().map(|change| self.perform_change(change)))
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
                    tasks.push(Task::done(Message::MarginChange { id: window, margin }));
                }
                if let Some(size) = size {
                    existing.size = size;
                    tasks.push(Task::done(Message::SizeChange { id: window, size }));
                }
                Task::batch(tasks)
            }
            SurfaceChange::Close(id) => match self.surfaces.remove(&id) {
                Some(existing) => Task::done(Message::RemoveWindow(existing.id)),
                None => Task::none(),
            },
        }
    }

    /// Each visible toast's placement: its output plus the anchor and margin its
    /// surface should take. Freshest hugs the edge, older siblings step inward;
    /// a centred vertical group is centred as a whole against the output height.
    /// Empty while DND is on.
    fn desired_surfaces(
        &self,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Vec<DesiredSurface> {
        if self.dnd {
            return Vec::new();
        }
        let width = config.notifications.width_px.round() as u32;
        // Newest first, so walking a corner group meets the freshest toast first.
        let mut order: Vec<&Notification> = self.live.iter().collect();
        order.sort_by_key(|n| std::cmp::Reverse(n.created_at));

        // Resolve each toast's placement and measured height once.
        let placed: Vec<(&Notification, ToastPlacement, f32)> = order
            .iter()
            .map(|notif| {
                let place = placement_for(&notif.data, placement, config);
                let height = self.heights.get(&notif.data.id).copied().unwrap_or(0.0);
                (*notif, place, height)
            })
            .collect();

        // Total height of each corner group (card heights plus the gaps between
        // them), so a centred stack can be centred as one block.
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
                let (anchor, margin) = stack::slot_placement(
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
                    size: (
                        width,
                        stack::surface_height(self.heights.get(&notif.data.id).copied()),
                    ),
                }
            })
            .collect()
    }
}

/// The corner a toast groups into: its output and both edges.
type GroupKey = (Option<String>, VEdge, HEdge);

fn group_key(place: &ToastPlacement) -> GroupKey {
    (place.output.clone(), place.vertical, place.horizontal)
}

/// One change to reconcile a toast's surface with the desired set. Produced by
/// [`NotificationGroup::plan_changes`], performed by
/// [`NotificationGroup::perform_change`].
enum SurfaceChange {
    Open(DesiredSurface),
    Adjust {
        id: u32,
        anchor: Option<Anchor>,
        margin: Option<(i32, i32, i32, i32)>,
        size: Option<(u32, u32)>,
    },
    Close(u32),
}

/// A toast's fully-resolved desired surface: where it goes and how big.
#[derive(Clone)]
struct DesiredSurface {
    id: u32,
    output: Option<String>,
    anchor: Anchor,
    margin: (i32, i32, i32, i32),
    size: (u32, u32),
}

/// Whether a notification's window has closed. Only "window-owned" toasts — one
/// with a resolvable window and no foreign action — go stale; a notification
/// carrying an allow/deny action is a question the client is blocked on and must
/// never be auto-dismissed.
fn is_stale(data: &NotificationData, placement: &Placement) -> bool {
    let Some(window) = resolve_window_id(data, placement) else {
        return false;
    };
    let window_owned = data.actions.iter().all(|a| is_window_action(&a.key));
    window_owned && placement.workspace_of_window(window).is_none()
}
