pub mod drag_ghost;
pub mod draggable_slot;
pub mod keyed_slot_list;

use std::{fmt::Debug, hash::Hash};

use iced::{
    Event, Point, Subscription,
    event::listen_with,
    mouse,
    time::{Duration, Instant},
    widget::container,
    window,
};

pub trait SlotItem: Copy + Eq + Hash + Debug + Send + 'static {
    type List: Copy + Eq + Debug + Send + 'static;

    fn list(self) -> Self::List;
}

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

impl<Item: SlotItem> ActiveDrag<Item> {
    pub fn reordered_to(&self) -> Option<usize> {
        match self.target {
            DragTarget::OwnList(to) if to != self.from => Some(to),
            DragTarget::OwnList(_) | DragTarget::ForeignSlot(_) | DragTarget::Outside => None,
        }
    }

    pub fn hovered(&self) -> Option<ForeignHover<Item>> {
        match self.target {
            DragTarget::ForeignSlot(hover) => Some(hover),
            DragTarget::OwnList(_) | DragTarget::Outside => None,
        }
    }

    fn left(self, list: Item::List) -> Self {
        let target = match self.target {
            DragTarget::OwnList(_) if self.item.list() == list => DragTarget::Outside,
            DragTarget::ForeignSlot(hover) if hover.item.list() == list => DragTarget::Outside,
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
pub enum DragEvent<Item: SlotItem, Click> {
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
    LeftList(Item::List),
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

pub fn advance<Item: SlotItem, Click: Copy>(
    drag: PointerDrag<Item, Click>,
    event: DragEvent<Item, Click>,
    start_distance: f32,
) -> DragTransition<Item, Click> {
    match event {
        DragEvent::Pressed(press) => PointerDrag::Pressed(press).into(),
        DragEvent::PointerMoved { surface, position } => {
            pointer_moved(drag, surface, position, start_distance)
        }
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
        DragEvent::EnteredSlot { item, position } => entered_slot(drag, item, position),
        DragEvent::LeftList(list) => match drag {
            PointerDrag::Dragging(active) => PointerDrag::Dragging(active.left(list)).into(),
            unaffected => unaffected.into(),
        },
    }
}

fn pointer_moved<Item: SlotItem, Click: Copy>(
    drag: PointerDrag<Item, Click>,
    surface: window::Id,
    position: Point,
    start_distance: f32,
) -> DragTransition<Item, Click> {
    match drag {
        PointerDrag::Pressed(press) => PointerDrag::Held {
            press,
            origin: position,
        }
        .into(),
        PointerDrag::Held { press, origin } if origin.distance(position) > start_distance => {
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

fn entered_slot<Item: SlotItem, Click: Copy>(
    drag: PointerDrag<Item, Click>,
    item: Item,
    position: usize,
) -> DragTransition<Item, Click> {
    let PointerDrag::Dragging(active) = drag else {
        return drag.into();
    };
    if item == active.item {
        return drag.into();
    }
    if item.list() == active.item.list() {
        return PointerDrag::Dragging(ActiveDrag {
            target: DragTarget::OwnList(position),
            ..active
        })
        .into();
    }
    let hover = ForeignHover {
        item,
        since: Instant::now(),
    };
    DragTransition {
        drag: PointerDrag::Dragging(ActiveDrag {
            target: DragTarget::ForeignSlot(hover),
            ..active
        }),
        outcome: DragOutcome::HoveringForeign(hover),
    }
}

pub fn pointer_events<Item: SlotItem, Click: Copy + Send + 'static>(
    tracks_moves: bool,
) -> Subscription<DragEvent<Item, Click>> {
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
    pub ghost_frame: container::Style,
}

impl<Item: SlotItem> DragView<Item> {
    pub fn new<Click>(drag: &PointerDrag<Item, Click>, ghost_frame: &container::Style) -> Self {
        Self {
            drag: match drag {
                PointerDrag::Dragging(active) => Some(*active),
                PointerDrag::Idle | PointerDrag::Pressed(_) | PointerDrag::Held { .. } => None,
            },
            ghost_frame: *ghost_frame,
        }
    }

    pub fn is_active(&self) -> bool {
        self.drag.is_some()
    }

    pub fn is_dragging(&self, item: Item) -> bool {
        self.drag.is_some_and(|drag| drag.item == item)
    }

    pub fn has_left_its_list(&self) -> bool {
        self.drag
            .is_some_and(|drag| !matches!(drag.target, DragTarget::OwnList(_)))
    }

    pub fn hovers(&self, item: Item) -> Option<ForeignHover<Item>> {
        self.drag?.hovered().filter(|hover| hover.item == item)
    }

    pub fn hover_charge(&self, item: Item, full_after: Duration) -> Option<f32> {
        let hover = self.hovers(item)?;
        Some(
            (hover.since.elapsed().as_secs_f32() / full_after.as_secs_f32().max(f32::EPSILON))
                .min(1.0),
        )
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
