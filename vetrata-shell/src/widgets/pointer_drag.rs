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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraggedBarSlot {
    Workspace(u64),
    Window { window: u64, workspace: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarSlotList {
    Workspaces,
    Windows,
}

impl DraggedBarSlot {
    fn list(self) -> BarSlotList {
        match self {
            DraggedBarSlot::Workspace(_) => BarSlotList::Workspaces,
            DraggedBarSlot::Window { .. } => BarSlotList::Windows,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SlotPress<Click> {
    pub item: DraggedBarSlot,
    pub position: usize,
    pub click: Click,
}

#[derive(Debug, Clone, Copy)]
pub enum PointerDrag<Click> {
    Idle,
    Pressed(SlotPress<Click>),
    Held {
        press: SlotPress<Click>,
        origin: Point,
    },
    Dragging(ActiveDrag),
}

impl<Click> PointerDrag<Click> {
    pub fn is_idle(&self) -> bool {
        matches!(self, PointerDrag::Idle)
    }

    pub fn is_dragging(&self) -> bool {
        matches!(self, PointerDrag::Dragging(_))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ActiveDrag {
    pub item: DraggedBarSlot,
    pub surface: window::Id,
    pub cursor: Point,
    pub from: usize,
    pub target: DragTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragTarget {
    OwnList(usize),
    ForeignSlot(ForeignHover),
    Outside,
}

impl ActiveDrag {
    #[must_use]
    pub fn reordered_to(&self) -> Option<usize> {
        match self.target {
            DragTarget::OwnList(to) if to != self.from => Some(to),
            DragTarget::OwnList(_) | DragTarget::ForeignSlot(_) | DragTarget::Outside => None,
        }
    }

    #[must_use]
    pub fn hovered(&self) -> Option<ForeignHover> {
        match self.target {
            DragTarget::ForeignSlot(hover) => Some(hover),
            DragTarget::OwnList(_) | DragTarget::Outside => None,
        }
    }

    fn left(self, list: BarSlotList) -> Self {
        let target = match self.target {
            DragTarget::OwnList(_) if self.item.list() == list => DragTarget::Outside,
            DragTarget::ForeignSlot(hover) if hover.item.list() == list => DragTarget::Outside,
            kept => kept,
        };
        Self { target, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignHover {
    pub item: DraggedBarSlot,
    pub since: Instant,
}

#[derive(Debug, Clone, Copy)]
pub enum DragEvent<Click> {
    Pressed(SlotPress<Click>),
    PointerMoved {
        surface: window::Id,
        position: Point,
    },
    Released,
    EnteredSlot {
        item: DraggedBarSlot,
        position: usize,
    },
    LeftList(BarSlotList),
}

#[derive(Debug, Clone, Copy)]
pub enum DragOutcome<Click> {
    Unchanged,
    Started,
    Clicked(Click),
    Dropped(ActiveDrag),
    HoveringForeign(ForeignHover),
}

pub struct DragTransition<Click> {
    pub drag: PointerDrag<Click>,
    pub outcome: DragOutcome<Click>,
}

impl<Click> From<PointerDrag<Click>> for DragTransition<Click> {
    fn from(drag: PointerDrag<Click>) -> Self {
        Self {
            drag,
            outcome: DragOutcome::Unchanged,
        }
    }
}

pub fn advance<Click: Copy>(
    drag: PointerDrag<Click>,
    event: DragEvent<Click>,
    start_distance: f32,
) -> DragTransition<Click> {
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

fn pointer_moved<Click: Copy>(
    drag: PointerDrag<Click>,
    surface: window::Id,
    position: Point,
    start_distance: f32,
) -> DragTransition<Click> {
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

fn entered_slot<Click: Copy>(
    drag: PointerDrag<Click>,
    item: DraggedBarSlot,
    position: usize,
) -> DragTransition<Click> {
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

pub fn pointer_events<Click: Copy + Send + 'static>(
    tracks_moves: bool,
) -> Subscription<DragEvent<Click>> {
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
pub struct DragView {
    pub drag: Option<ActiveDrag>,
    pub ghost_frame: container::Style,
}

impl DragView {
    pub fn new<Click>(drag: &PointerDrag<Click>, ghost_frame: &container::Style) -> Self {
        Self {
            drag: match drag {
                PointerDrag::Dragging(active) => Some(*active),
                PointerDrag::Idle | PointerDrag::Pressed(_) | PointerDrag::Held { .. } => None,
            },
            ghost_frame: *ghost_frame,
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.drag.is_some()
    }

    #[must_use]
    pub fn is_dragging(&self, item: DraggedBarSlot) -> bool {
        self.drag.is_some_and(|drag| drag.item == item)
    }

    #[must_use]
    pub fn has_left_its_list(&self) -> bool {
        self.drag
            .is_some_and(|drag| !matches!(drag.target, DragTarget::OwnList(_)))
    }

    #[must_use]
    pub fn hovers(&self, item: DraggedBarSlot) -> Option<ForeignHover> {
        self.drag?.hovered().filter(|hover| hover.item == item)
    }

    #[must_use]
    pub fn hover_charge(&self, item: DraggedBarSlot, full_after: Duration) -> Option<f32> {
        let hover = self.hovers(item)?;
        Some(
            (hover.since.elapsed().as_secs_f32() / full_after.as_secs_f32().max(f32::EPSILON))
                .min(1.0),
        )
    }

    pub fn previewed<'s, T>(
        &self,
        items: &'s [T],
        identity: impl Fn(&T) -> DraggedBarSlot,
    ) -> Vec<&'s T> {
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
