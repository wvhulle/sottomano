pub mod drag_ghost;
pub mod draggable_slot;
pub mod slot_group;
pub mod slot_pager;

use iced::{
    Event, Point, Subscription,
    event::listen_with,
    mouse,
    time::{Duration, Instant},
    window,
};

use crate::frame_style::FrameStyle;

pub const DRAG_START_DISTANCE_PX: f32 = 6.0;

#[derive(Debug, Clone, Copy)]
pub struct SlotPress<Item, Click> {
    pub item: Item,
    pub position: usize,
    pub click: Click,
}

#[derive(Debug, Clone, Copy)]
pub enum PointerDrag<Item, Click> {
    Idle,
    Pressed(SlotPress<Item, Click>),
    Held {
        press: SlotPress<Item, Click>,
        origin: Point,
    },
    Dragging(ActiveDrag<Item>),
}

impl<Item, Click> PointerDrag<Item, Click> {
    pub fn is_idle(&self) -> bool {
        matches!(self, PointerDrag::Idle)
    }

    pub fn is_dragging(&self) -> bool {
        matches!(self, PointerDrag::Dragging(_))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ActiveDrag<Item> {
    pub item: Item,
    pub surface: window::Id,
    pub cursor: Point,
    pub from: usize,
    pub target: DragTarget<Item>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget<Item> {
    OwnList(usize),
    ForeignSlot(ForeignHover<Item>),
    Outside,
}

impl<Item: Copy> ActiveDrag<Item> {
    #[must_use]
    pub fn reordered_to(&self) -> Option<usize> {
        match self.target {
            DragTarget::OwnList(to) if to != self.from => Some(to),
            DragTarget::OwnList(_) | DragTarget::ForeignSlot(_) | DragTarget::Outside => None,
        }
    }

    #[must_use]
    pub fn hovered(&self) -> Option<ForeignHover<Item>> {
        match self.target {
            DragTarget::ForeignSlot(hover) => Some(hover),
            DragTarget::OwnList(_) | DragTarget::Outside => None,
        }
    }

    fn left<List: PartialEq>(self, list: &List, list_of: &impl Fn(Item) -> List) -> Self {
        let target = match self.target {
            DragTarget::OwnList(_) if list_of(self.item) == *list => DragTarget::Outside,
            DragTarget::ForeignSlot(hover) if list_of(hover.item) == *list => DragTarget::Outside,
            kept => kept,
        };
        Self { target, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignHover<Item> {
    pub item: Item,
    pub since: Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum DragEvent<Item, List, Click> {
    Pressed(SlotPress<Item, Click>),
    PointerMoved {
        surface: window::Id,
        position: Point,
    },
    Released,
    EnteredSlot {
        item: Item,
        position: usize,
    },
    LeftList(List),
}

#[derive(Debug, Clone, Copy)]
pub enum DragOutcome<Item, Click> {
    Unchanged,
    Started,
    Clicked(Click),
    Dropped(ActiveDrag<Item>),
    HoveringForeign(ForeignHover<Item>),
}

pub struct DragTransition<Item, Click> {
    pub drag: PointerDrag<Item, Click>,
    pub outcome: DragOutcome<Item, Click>,
}

impl<Item, Click> From<PointerDrag<Item, Click>> for DragTransition<Item, Click> {
    fn from(drag: PointerDrag<Item, Click>) -> Self {
        Self {
            drag,
            outcome: DragOutcome::Unchanged,
        }
    }
}

pub fn advance<Item: Copy + PartialEq, List: PartialEq, Click: Copy>(
    drag: PointerDrag<Item, Click>,
    event: DragEvent<Item, List, Click>,
    now: Instant,
    list_of: impl Fn(Item) -> List,
) -> DragTransition<Item, Click> {
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

pub fn advance_within_one_list<Item: Copy + PartialEq, Click: Copy>(
    drag: PointerDrag<Item, Click>,
    event: DragEvent<Item, (), Click>,
    now: Instant,
) -> DragTransition<Item, Click> {
    advance(drag, event, now, |_| ())
}

fn pointer_moved<Item: Copy, Click: Copy>(
    drag: PointerDrag<Item, Click>,
    surface: window::Id,
    position: Point,
) -> DragTransition<Item, Click> {
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

fn entered_slot<Item: Copy + PartialEq, List: PartialEq, Click: Copy>(
    drag: PointerDrag<Item, Click>,
    item: Item,
    position: usize,
    now: Instant,
    list_of: &impl Fn(Item) -> List,
) -> DragTransition<Item, Click> {
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

pub fn pointer_events<Item: Send + 'static, List: Send + 'static, Click: Copy + Send + 'static>(
    tracks_moves: bool,
) -> Subscription<DragEvent<Item, List, Click>> {
    let releases = listen_with(|event, _status, _surface| match event {
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(DragEvent::Released)
        }
        _ => None,
    });
    if !tracks_moves {
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
pub struct DragView<Item> {
    pub drag: Option<ActiveDrag<Item>>,
    pub ghost_frame: FrameStyle,
}

impl<Item: Copy + PartialEq> DragView<Item> {
    pub fn new<Click>(drag: &PointerDrag<Item, Click>, ghost_frame: FrameStyle) -> Self {
        Self {
            drag: match drag {
                PointerDrag::Dragging(active) => Some(*active),
                PointerDrag::Idle | PointerDrag::Pressed(_) | PointerDrag::Held { .. } => None,
            },
            ghost_frame,
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.drag.is_some()
    }

    #[must_use]
    pub fn is_dragging(&self, item: Item) -> bool {
        self.drag.is_some_and(|drag| drag.item == item)
    }

    #[must_use]
    pub fn has_left_its_list(&self) -> bool {
        self.drag
            .is_some_and(|drag| !matches!(drag.target, DragTarget::OwnList(_)))
    }

    #[must_use]
    pub fn hovers(&self, item: Item) -> Option<ForeignHover<Item>> {
        self.drag?.hovered().filter(|hover| hover.item == item)
    }

    #[must_use]
    pub fn hover_charge(&self, item: Item, full_after: Duration, now: Instant) -> Option<f32> {
        let hover = self.hovers(item)?;
        let held = now.saturating_duration_since(hover.since);
        Some((held.as_secs_f32() / full_after.as_secs_f32().max(f32::EPSILON)).min(1.0))
    }

    pub fn previewed<'s, T>(&self, items: &'s [T], identity: impl Fn(&T) -> Item) -> Vec<&'s T> {
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
        let pressed = advance_within_one_list(PointerDrag::Idle, DragEvent::Pressed(PRESS), now);
        let move_to = |drag, position| {
            advance_within_one_list(
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
        let entered = advance_within_one_list(
            dragged_past_threshold(now),
            DragEvent::EnteredSlot {
                item: 2,
                position: 3,
            },
            now,
        );
        let released = advance_within_one_list(entered.drag, DragEvent::Released, now);
        assert!(matches!(
            released.outcome,
            DragOutcome::Dropped(dropped) if dropped.reordered_to() == Some(3)
        ));
    }

    #[test]
    fn a_press_without_movement_is_a_click() {
        let now = Instant::now();
        let pressed = advance_within_one_list(PointerDrag::Idle, DragEvent::Pressed(PRESS), now);
        let released = advance_within_one_list(pressed.drag, DragEvent::Released, now);
        assert!(matches!(released.outcome, DragOutcome::Clicked(())));
    }

    #[test]
    fn the_preview_moves_the_dragged_item_to_its_target() {
        let now = Instant::now();
        let entered = advance_within_one_list(
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
