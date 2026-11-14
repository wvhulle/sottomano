mod drag_ghost;
mod draggable_slot;
mod list_end_corners;
mod reorderable_list;
mod slot_group;

pub use drag_ghost::{GhostPlacement, drag_ghost};
pub use draggable_slot::{HoverReorder, draggable_slot};
use iced::{
    Event, Point, Subscription,
    event::listen_with,
    mouse,
    time::{Duration, Instant},
    window,
};
pub use list_end_corners::ListEndCorners;
pub use reorderable_list::{ReorderableList, SlotAlarm, UngroupedHighlightLook, reorderable_list};
pub use slot_group::{SlotGroup, SlotTileInset};

use crate::theming::FrameStyle;

pub const DRAG_START_DISTANCE_PX: f32 = 6.0;

#[derive(Debug, Clone, Copy)]
pub struct SlotPress<SlotId, ClickPayload> {
    pub item: SlotId,
    pub position: usize,
    pub click: ClickPayload,
}

#[derive(Debug, Clone, Copy)]
pub enum PointerDrag<SlotId, ClickPayload> {
    Idle,
    Pressed(SlotPress<SlotId, ClickPayload>),
    Held {
        press: SlotPress<SlotId, ClickPayload>,
        origin: Point,
    },
    Dragging(ActiveDrag<SlotId>),
}

#[derive(Debug, Clone, Copy)]
pub struct ActiveDrag<SlotId> {
    pub item: SlotId,
    pub surface: window::Id,
    pub cursor: Point,
    pub from: usize,
    pub target: DragTarget<SlotId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget<SlotId> {
    OwnList(usize),
    ForeignSlot(ForeignHover<SlotId>),
    Outside,
}

impl<SlotId: Copy> ActiveDrag<SlotId> {
    #[must_use]
    pub fn reordered_to(&self) -> Option<usize> {
        match self.target {
            DragTarget::OwnList(to) if to != self.from => Some(to),
            DragTarget::OwnList(_) | DragTarget::ForeignSlot(_) | DragTarget::Outside => None,
        }
    }

    #[must_use]
    pub fn hovered(&self) -> Option<ForeignHover<SlotId>> {
        match self.target {
            DragTarget::ForeignSlot(hover) => Some(hover),
            DragTarget::OwnList(_) | DragTarget::Outside => None,
        }
    }

    fn left<ListId: PartialEq>(self, list: &ListId, list_of: &impl Fn(SlotId) -> ListId) -> Self {
        let target = match self.target {
            DragTarget::OwnList(_) if list_of(self.item) == *list => DragTarget::Outside,
            DragTarget::ForeignSlot(hover) if list_of(hover.item) == *list => DragTarget::Outside,
            kept => kept,
        };
        Self { target, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignHover<SlotId> {
    pub item: SlotId,
    pub since: Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum DragEvent<SlotId, ListId, ClickPayload> {
    Pressed(SlotPress<SlotId, ClickPayload>),
    PointerMoved {
        surface: window::Id,
        position: Point,
    },
    Released,
    EnteredSlot {
        item: SlotId,
        position: usize,
    },
    LeftList(ListId),
}

#[derive(Debug, Clone, Copy)]
pub enum DragOutcome<SlotId, ClickPayload> {
    Unchanged,
    Started,
    Clicked(ClickPayload),
    Dropped(ActiveDrag<SlotId>),
    HoveringForeign(ForeignHover<SlotId>),
}

pub struct DragTransition<SlotId, ClickPayload> {
    pub drag: PointerDrag<SlotId, ClickPayload>,
    pub outcome: DragOutcome<SlotId, ClickPayload>,
}

impl<SlotId, ClickPayload> From<PointerDrag<SlotId, ClickPayload>>
    for DragTransition<SlotId, ClickPayload>
{
    fn from(drag: PointerDrag<SlotId, ClickPayload>) -> Self {
        Self {
            drag,
            outcome: DragOutcome::Unchanged,
        }
    }
}

pub fn advance_drag<SlotId: Copy + PartialEq, ListId: PartialEq, ClickPayload: Copy>(
    drag: PointerDrag<SlotId, ClickPayload>,
    event: DragEvent<SlotId, ListId, ClickPayload>,
    now: Instant,
    list_of: impl Fn(SlotId) -> ListId,
) -> DragTransition<SlotId, ClickPayload> {
    match event {
        DragEvent::Pressed(press) => PointerDrag::Pressed(press).into(),
        DragEvent::PointerMoved { surface, position } => pointer_moved(drag, surface, position),
        DragEvent::Released => DragTransition {
            drag: PointerDrag::Idle,
            outcome: match drag {
                PointerDrag::Pressed(press) | PointerDrag::Held { press, .. } => {
                    DragOutcome::Clicked(press.click)
                }
                PointerDrag::Dragging(active) => DragOutcome::Dropped(active),
                PointerDrag::Idle => DragOutcome::Unchanged,
            },
        },
        DragEvent::EnteredSlot { item, position } => {
            entered_slot(drag, item, position, now, &list_of)
        }
        DragEvent::LeftList(list) => match drag {
            PointerDrag::Dragging(active) => {
                PointerDrag::Dragging(active.left(&list, &list_of)).into()
            }
            unaffected => unaffected.into(),
        },
    }
}

pub fn advance_drag_within_list<SlotId: Copy + PartialEq, ClickPayload: Copy>(
    drag: PointerDrag<SlotId, ClickPayload>,
    event: DragEvent<SlotId, (), ClickPayload>,
    now: Instant,
) -> DragTransition<SlotId, ClickPayload> {
    advance_drag(drag, event, now, |_| ())
}

fn pointer_moved<SlotId: Copy, ClickPayload: Copy>(
    drag: PointerDrag<SlotId, ClickPayload>,
    surface: window::Id,
    position: Point,
) -> DragTransition<SlotId, ClickPayload> {
    match drag {
        PointerDrag::Pressed(press) => PointerDrag::Held {
            press,
            origin: position,
        }
        .into(),
        PointerDrag::Held { press, origin }
            if origin.distance(position) > DRAG_START_DISTANCE_PX =>
        {
            DragTransition {
                drag: PointerDrag::Dragging(ActiveDrag {
                    item: press.item,
                    surface,
                    cursor: position,
                    from: press.position,
                    target: DragTarget::OwnList(press.position),
                }),
                outcome: DragOutcome::Started,
            }
        }
        PointerDrag::Dragging(active) => PointerDrag::Dragging(ActiveDrag {
            cursor: position,
            ..active
        })
        .into(),
        unmoved @ (PointerDrag::Idle | PointerDrag::Held { .. }) => unmoved.into(),
    }
}

fn entered_slot<SlotId: Copy + PartialEq, ListId: PartialEq, ClickPayload: Copy>(
    drag: PointerDrag<SlotId, ClickPayload>,
    item: SlotId,
    position: usize,
    now: Instant,
    list_of: &impl Fn(SlotId) -> ListId,
) -> DragTransition<SlotId, ClickPayload> {
    let PointerDrag::Dragging(active) = drag else {
        return drag.into();
    };
    if item == active.item {
        return drag.into();
    }
    if list_of(item) == list_of(active.item) {
        return PointerDrag::Dragging(ActiveDrag {
            target: DragTarget::OwnList(position),
            ..active
        })
        .into();
    }
    let hover = ForeignHover { item, since: now };
    DragTransition {
        drag: PointerDrag::Dragging(ActiveDrag {
            target: DragTarget::ForeignSlot(hover),
            ..active
        }),
        outcome: DragOutcome::HoveringForeign(hover),
    }
}

pub fn drag_pointer_events<
    SlotId: Send + 'static,
    ListId: Send + 'static,
    ClickPayload: Copy + Send + 'static,
>(
    move_tracking: MoveTracking,
) -> Subscription<DragEvent<SlotId, ListId, ClickPayload>> {
    let releases = listen_with(|event, _status, _surface| match event {
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(DragEvent::Released)
        }
        _ => None,
    });
    if move_tracking == MoveTracking::Untracked {
        return releases;
    }
    let moves = listen_with(|event, _status, surface| match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Some(DragEvent::PointerMoved { surface, position })
        }
        _ => None,
    });
    Subscription::batch([releases, moves])
}

#[derive(Debug, Clone, Copy)]
pub struct DragView<SlotId> {
    pub drag: Option<ActiveDrag<SlotId>>,
    pub ghost_frame: FrameStyle,
}

impl<SlotId: Copy + PartialEq> DragView<SlotId> {
    pub fn new<ClickPayload>(
        drag: &PointerDrag<SlotId, ClickPayload>,
        ghost_frame: FrameStyle,
    ) -> Self {
        Self {
            drag: match drag {
                PointerDrag::Dragging(active) => Some(*active),
                PointerDrag::Idle | PointerDrag::Pressed(_) | PointerDrag::Held { .. } => None,
            },
            ghost_frame,
        }
    }

    #[must_use]
    pub fn list_departure(&self) -> ListDeparture {
        match self.drag {
            Some(active_drag) if !matches!(active_drag.target, DragTarget::OwnList(_)) => {
                ListDeparture::LeftOwnList
            }
            Some(_) | None => ListDeparture::InOwnList,
        }
    }

    #[must_use]
    pub fn hovers(&self, item: SlotId) -> Option<ForeignHover<SlotId>> {
        self.drag?.hovered().filter(|hover| hover.item == item)
    }

    #[must_use]
    pub fn hover_charge(&self, item: SlotId, full_after: Duration, now: Instant) -> Option<f32> {
        let hover = self.hovers(item)?;
        let held = now
            .checked_duration_since(hover.since)
            .expect("a foreign hover starts no later than the clock that charges it");
        Some((held.as_secs_f32() / full_after.as_secs_f32().max(f32::EPSILON)).min(1.0))
    }

    pub fn previewed<'s, T>(&self, items: &'s [T], identity: impl Fn(&T) -> SlotId) -> Vec<&'s T> {
        let mut order: Vec<&T> = items.iter().collect();
        let Some(drag) = self.drag else {
            return order;
        };
        let DragTarget::OwnList(to) = drag.target else {
            return order;
        };
        let Some(from) = order.iter().position(|item| identity(item) == drag.item) else {
            return order;
        };
        let moved = order.remove(from);
        order.insert(to.min(order.len()), moved);
        order
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    const PRESS: SlotPress<u32, ()> = SlotPress {
        item: 1,
        position: 0,
        click: (),
    };

    fn dragged_past_threshold(now: Instant) -> PointerDrag<u32, ()> {
        let pressed = advance_drag_within_list(PointerDrag::Idle, DragEvent::Pressed(PRESS), now);
        let move_to = |drag, position| {
            advance_drag_within_list(
                drag,
                DragEvent::PointerMoved {
                    surface: window::Id::unique(),
                    position,
                },
                now,
            )
            .drag
        };
        let held = move_to(pressed.drag, Point::ORIGIN);
        move_to(held, Point::new(50.0, 0.0))
    }

    #[test]
    fn entering_a_slot_of_a_single_list_reorders_on_release() {
        let now = Instant::now();
        let entered = advance_drag_within_list(
            dragged_past_threshold(now),
            DragEvent::EnteredSlot {
                item: 2,
                position: 3,
            },
            now,
        );
        let released = advance_drag_within_list(entered.drag, DragEvent::Released, now);
        assert!(matches!(
            released.outcome,
            DragOutcome::Dropped(dropped) if dropped.reordered_to() == Some(3)
        ));
    }

    #[test]
    fn a_press_without_movement_is_a_click() {
        let now = Instant::now();
        let pressed = advance_drag_within_list(PointerDrag::Idle, DragEvent::Pressed(PRESS), now);
        let released = advance_drag_within_list(pressed.drag, DragEvent::Released, now);
        assert!(matches!(released.outcome, DragOutcome::Clicked(())));
    }

    #[test]
    fn the_preview_moves_the_dragged_item_to_its_target() {
        let now = Instant::now();
        let entered = advance_drag_within_list(
            dragged_past_threshold(now),
            DragEvent::EnteredSlot {
                item: 3,
                position: 2,
            },
            now,
        );
        let view = DragView::new(&entered.drag, FrameStyle::default());
        let order = view.previewed(&[1_u32, 2, 3], |item| *item);
        assert_eq!(order, [&2, &3, &1]);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveTracking {
    Tracked,
    Untracked,
}

impl<SlotId, ClickPayload> From<&PointerDrag<SlotId, ClickPayload>> for MoveTracking {
    fn from(pointer_drag: &PointerDrag<SlotId, ClickPayload>) -> Self {
        match pointer_drag {
            PointerDrag::Idle => MoveTracking::Untracked,
            PointerDrag::Pressed(_) | PointerDrag::Held { .. } | PointerDrag::Dragging(_) => {
                MoveTracking::Tracked
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListDeparture {
    InOwnList,
    LeftOwnList,
}
