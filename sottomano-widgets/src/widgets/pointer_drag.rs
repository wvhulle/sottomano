pub mod slot_pager;

use std::{fmt::Debug, hash::Hash};

use corredo::pointer_drag as generic_drag;
pub use corredo::pointer_drag::{drag_ghost, draggable_slot, slot_group};
use iced::{Subscription, time::Instant};

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

pub type SlotPress<Click> = generic_drag::SlotPress<DraggedPanelSlot, Click>;
pub type PointerDrag<Click> = generic_drag::PointerDrag<DraggedPanelSlot, Click>;
pub type ActiveDrag = generic_drag::ActiveDrag<DraggedPanelSlot>;
pub type DragTarget = generic_drag::DragTarget<DraggedPanelSlot>;
pub type ForeignHover = generic_drag::ForeignHover<DraggedPanelSlot>;
pub type DragEvent<Click> = generic_drag::DragEvent<DraggedPanelSlot, PanelSlotList, Click>;
pub type DragOutcome<Click> = generic_drag::DragOutcome<DraggedPanelSlot, Click>;
pub type DragTransition<Click> = generic_drag::DragTransition<DraggedPanelSlot, Click>;
pub type DragView = generic_drag::DragView<DraggedPanelSlot>;

pub fn advance<Click: Copy>(
    drag: PointerDrag<Click>,
    event: DragEvent<Click>,
    now: Instant,
) -> DragTransition<Click> {
    generic_drag::advance(drag, event, now, DraggedPanelSlot::list)
}

pub fn pointer_events<Click: Copy + Send + 'static>(
    tracks_moves: bool,
) -> Subscription<DragEvent<Click>> {
    generic_drag::pointer_events(tracks_moves)
}

#[cfg(test)]
mod tests {
    use corredo::pointer_drag::DRAG_START_DISTANCE_PX;
    use iced::{Point, window};

    use super::*;
    use crate::clock::Clock;

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
        let pressed = advance(PointerDrag::Idle, DragEvent::Pressed(press), now);
        let held = advance(
            pressed.drag,
            DragEvent::PointerMoved {
                surface,
                position: Point::ORIGIN,
            },
            now,
        );
        let started = advance(
            held.drag,
            DragEvent::PointerMoved {
                surface,
                position: Point::new(DRAG_START_DISTANCE_PX * 2.0, 0.0),
            },
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
            now,
        );
        let released = advance(entered.drag, DragEvent::Released, now);
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
            now,
        );
        assert!(matches!(
            entered.outcome,
            DragOutcome::HoveringForeign(ForeignHover {
                item: DraggedPanelSlot::Workspace(2),
                ..
            })
        ));
        let released = advance(entered.drag, DragEvent::Released, now);
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
            now,
        );
        let left = advance(
            entered.drag,
            DragEvent::LeftList(PanelSlotList::Windows),
            now,
        );
        let released = advance(left.drag, DragEvent::Released, now);
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
        let pressed = advance(PointerDrag::Idle, DragEvent::Pressed(press), now);
        let released = advance(pressed.drag, DragEvent::Released, now);
        assert!(matches!(released.outcome, DragOutcome::Clicked(())));
    }
}
