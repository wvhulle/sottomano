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

use std::{collections::HashMap, time::SystemTime};

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
    /// open a surface per toast, keep each anchored/margined/sized to its
    /// computed corner slot, and close surfaces for gone (or DND-suppressed)
    /// toasts. Emits only the changes each surface actually needs.
    fn reconcile(
        &mut self,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Task<Message> {
        let width = config.notifications.width_px.round() as u32;
        let desired = self.desired_slots(placement, outputs, config);
        let live_ids: std::collections::HashSet<u32> = desired.iter().map(|s| s.id).collect();

        let mut tasks = Vec::new();

        for slot in &desired {
            let anchor = slot.anchor;
            let margin = slot.margin;
            let size = (width, stack::surface_height(self.heights.get(&slot.id).copied()));

            match self.surfaces.get_mut(&slot.id) {
                None => {
                    let id = Id::unique();
                    self.surfaces.insert(
                        slot.id,
                        ToastSurface {
                            id,
                            output: slot.output.clone(),
                            anchor,
                            margin,
                            size,
                        },
                    );
                    let settings =
                        stack::toast_settings(slot.output.as_deref(), anchor, margin, size);
                    tasks.push(Task::done(Message::NewLayerShell { settings, id }));
                }
                Some(surface) if surface.output != slot.output => {
                    // A surface cannot move between outputs; close and reopen.
                    tasks.push(Task::done(Message::RemoveWindow(surface.id)));
                    let id = Id::unique();
                    surface.id = id;
                    surface.output = slot.output.clone();
                    surface.anchor = anchor;
                    surface.margin = margin;
                    surface.size = size;
                    let settings =
                        stack::toast_settings(slot.output.as_deref(), anchor, margin, size);
                    tasks.push(Task::done(Message::NewLayerShell { settings, id }));
                }
                Some(surface) => {
                    if surface.anchor != anchor {
                        surface.anchor = anchor;
                        tasks.push(Task::done(Message::AnchorChange {
                            id: surface.id,
                            anchor,
                        }));
                    }
                    if surface.margin != margin {
                        surface.margin = margin;
                        tasks.push(Task::done(Message::MarginChange {
                            id: surface.id,
                            margin,
                        }));
                    }
                    if surface.size != size {
                        surface.size = size;
                        tasks.push(Task::done(Message::SizeChange {
                            id: surface.id,
                            size,
                        }));
                    }
                }
            }
        }

        // Close surfaces for toasts that are gone or DND-suppressed.
        let stale: Vec<u32> = self
            .surfaces
            .keys()
            .copied()
            .filter(|id| !live_ids.contains(id))
            .collect();
        for id in stale {
            if let Some(surface) = self.surfaces.remove(&id) {
                tasks.push(Task::done(Message::RemoveWindow(surface.id)));
            }
        }

        Task::batch(tasks)
    }

    /// Each visible toast's placement: its output plus the anchor and margin its
    /// surface should take. Freshest hugs the edge, older siblings step inward;
    /// a centred vertical group is centred as a whole against the output height.
    /// Empty while DND is on.
    fn desired_slots(
        &self,
        placement: &Placement,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
    ) -> Vec<Slot> {
        if self.dnd {
            return Vec::new();
        }
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
                Slot {
                    id: notif.data.id,
                    output: place.output.clone(),
                    anchor,
                    margin,
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

struct Slot {
    id: u32,
    output: Option<String>,
    anchor: Anchor,
    margin: (i32, i32, i32, i32),
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
