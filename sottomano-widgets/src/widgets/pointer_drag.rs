pub mod drag_ghost;
pub mod draggable_slot;
pub mod slot_pager;

use std::{fmt::Debug, hash::Hash};

use iced::{
    Event, Point, Subscription,
    event::listen_with,
    mouse,
    time::{Duration, Instant},
    window,
};

use crate::desktop_panels::panel_frame::LayerFrameStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraggedPanelSlot {
    Workspace(u64),
    Window { window: u64, workspace: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSlotList {
    Workspaces,
    Windows,
}

impl DraggedPanelSlot {
    fn list(self) -> PanelSlotList {
        match self {
            DraggedPanelSlot::Workspace(_) => PanelSlotList::Workspaces,
            DraggedPanelSlot::Window { .. } => PanelSlotList::Windows,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SlotPress<Click> {
    pub item: DraggedPanelSlot,
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
    pub item: DraggedPanelSlot,
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

    fn left(self, list: PanelSlotList) -> Self {
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
    pub item: DraggedPanelSlot,
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
        item: DraggedPanelSlot,
        position: usize,
    },
    LeftList(PanelSlotList),
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
    now: Instant,
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
        DragEvent::EnteredSlot { item, position } => entered_slot(drag, item, position, now),
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
    item: DraggedPanelSlot,
    position: usize,
    now: Instant,
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
    let hover = ForeignHover { item, since: now };
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
    pub ghost_frame: LayerFrameStyle,
}

impl DragView {
    pub fn new<Click>(drag: &PointerDrag<Click>, ghost_frame: LayerFrameStyle) -> Self {
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
    pub fn is_dragging(&self, item: DraggedPanelSlot) -> bool {
        self.drag.is_some_and(|drag| drag.item == item)
    }

    #[must_use]
    pub fn has_left_its_list(&self) -> bool {
        self.drag
            .is_some_and(|drag| !matches!(drag.target, DragTarget::OwnList(_)))
    }

    #[must_use]
    pub fn hovers(&self, item: DraggedPanelSlot) -> Option<ForeignHover> {
        self.drag?.hovered().filter(|hover| hover.item == item)
    }

    #[must_use]
    pub fn hover_charge(
        &self,
        item: DraggedPanelSlot,
        full_after: Duration,
        now: Instant,
    ) -> Option<f32> {
        let hover = self.hovers(item)?;
        let held = now.saturating_duration_since(hover.since);
        Some((held.as_secs_f32() / full_after.as_secs_f32().max(f32::EPSILON)).min(1.0))
    }

    pub fn previewed<'s, T>(
        &self,
        items: &'s [T],
        identity: impl Fn(&T) -> DraggedPanelSlot,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;

    const START_DISTANCE: f32 = 4.0;

    fn first_window() -> DraggedPanelSlot {
        DraggedPanelSlot::Window {
            window: 10,
            workspace: 1,
        }
    }

    fn third_window() -> DraggedPanelSlot {
        DraggedPanelSlot::Window {
            window: 30,
            workspace: 1,
        }
    }

    fn dragging_first_window(surface: window::Id, now: Instant) -> PointerDrag<()> {
        let press = SlotPress {
            item: first_window(),
            position: 0,
            click: (),
        };
        let pressed = advance(
            PointerDrag::Idle,
            DragEvent::Pressed(press),
            START_DISTANCE,
            now,
        );
        let held = advance(
            pressed.drag,
            DragEvent::PointerMoved {
                surface,
                position: Point::ORIGIN,
            },
            START_DISTANCE,
            now,
        );
        let started = advance(
            held.drag,
            DragEvent::PointerMoved {
                surface,
                position: Point::new(START_DISTANCE * 2.0, 0.0),
            },
            START_DISTANCE,
            now,
        );
        assert!(matches!(started.outcome, DragOutcome::Started));
        started.drag
    }

    fn dropped_drag(outcome: DragOutcome<()>) -> ActiveDrag {
        match outcome {
            DragOutcome::Dropped(active) => active,
            other => panic!("expected a drop, got {other:?}"),
        }
    }

    #[test]
    fn entering_a_slot_of_the_own_list_reorders_on_release() {
        let now = Clock::real().instant;
        let dragging = dragging_first_window(window::Id::unique(), now);
        let entered = advance(
            dragging,
            DragEvent::EnteredSlot {
                item: third_window(),
                position: 2,
            },
            START_DISTANCE,
            now,
        );
        let released = advance(entered.drag, DragEvent::Released, START_DISTANCE, now);
        assert_eq!(dropped_drag(released.outcome).reordered_to(), Some(2));
    }

    #[test]
    fn entering_a_workspace_while_dragging_a_window_hovers_it() {
        let now = Clock::real().instant;
        let dragging = dragging_first_window(window::Id::unique(), now);
        let entered = advance(
            dragging,
            DragEvent::EnteredSlot {
                item: DraggedPanelSlot::Workspace(2),
                position: 1,
            },
            START_DISTANCE,
            now,
        );
        assert!(matches!(
            entered.outcome,
            DragOutcome::HoveringForeign(ForeignHover {
                item: DraggedPanelSlot::Workspace(2),
                ..
            })
        ));
        let released = advance(entered.drag, DragEvent::Released, START_DISTANCE, now);
        assert_eq!(dropped_drag(released.outcome).reordered_to(), None);
    }

    #[test]
    fn leaving_the_own_list_cancels_the_reorder() {
        let now = Clock::real().instant;
        let dragging = dragging_first_window(window::Id::unique(), now);
        let entered = advance(
            dragging,
            DragEvent::EnteredSlot {
                item: third_window(),
                position: 2,
            },
            START_DISTANCE,
            now,
        );
        let left = advance(
            entered.drag,
            DragEvent::LeftList(PanelSlotList::Windows),
            START_DISTANCE,
            now,
        );
        let released = advance(left.drag, DragEvent::Released, START_DISTANCE, now);
        let dropped = dropped_drag(released.outcome);
        assert_eq!(dropped.target, DragTarget::Outside);
        assert_eq!(dropped.reordered_to(), None);
    }

    #[test]
    fn a_short_press_is_a_click() {
        let now = Clock::real().instant;
        let press = SlotPress {
            item: first_window(),
            position: 0,
            click: (),
        };
        let pressed = advance(
            PointerDrag::Idle,
            DragEvent::Pressed(press),
            START_DISTANCE,
            now,
        );
        let released = advance(pressed.drag, DragEvent::Released, START_DISTANCE, now);
        assert!(matches!(released.outcome, DragOutcome::Clicked(())));
    }
}
