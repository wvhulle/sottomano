use iced::time::{Duration, Instant};

use super::{ActiveDrag, DragTarget, ForeignHover, ListDeparture, PointerDrag};
use crate::theming::FrameStyle;

#[derive(Debug, Clone, Copy)]
pub enum DragActivity<SlotId> {
    Idle,
    Dragging(ActiveDrag<SlotId>),
}

#[derive(Debug, Clone, Copy)]
pub struct DragView<SlotId> {
    pub drag: DragActivity<SlotId>,
    pub ghost_frame: FrameStyle,
}

impl<SlotId: Copy + PartialEq> DragView<SlotId> {
    pub fn new<ClickPayload>(
        drag: &PointerDrag<SlotId, ClickPayload>,
        ghost_frame: FrameStyle,
    ) -> Self {
        Self {
            drag: match drag {
                PointerDrag::Dragging(active) => DragActivity::Dragging(*active),
                PointerDrag::Idle | PointerDrag::Pressed(_) | PointerDrag::Held { .. } => {
                    DragActivity::Idle
                }
            },
            ghost_frame,
        }
    }

    #[must_use]
    pub fn list_departure(&self) -> ListDeparture {
        match self.drag {
            DragActivity::Dragging(active_drag)
                if !matches!(active_drag.target, DragTarget::OwnList(_)) =>
            {
                ListDeparture::LeftOwnList
            }
            DragActivity::Dragging(_) | DragActivity::Idle => ListDeparture::InOwnList,
        }
    }

    #[must_use]
    pub fn foreign_hover(&self) -> Option<ForeignHover<SlotId>> {
        let DragActivity::Dragging(active_drag) = self.drag else {
            return None;
        };
        active_drag.hovered()
    }

    #[must_use]
    pub fn hovers(&self, item: SlotId) -> Option<ForeignHover<SlotId>> {
        self.foreign_hover().filter(|hover| hover.item == item)
    }

    #[must_use]
    pub fn hover_charge(&self, item: SlotId, full_after: Duration, now: Instant) -> Option<f32> {
        let hover = self.hovers(item)?;
        let held = now
            .checked_duration_since(hover.since)
            .expect("a foreign hover starts no later than the clock that charges it");
        Some((held.as_secs_f32() / full_after.as_secs_f32().max(f32::EPSILON)).min(1.0))
    }

    pub fn previewed<'items, T>(
        &self,
        items: &'items [T],
        identity: impl Fn(&T) -> SlotId,
    ) -> Vec<&'items T> {
        let mut order: Vec<&T> = items.iter().collect();
        let DragActivity::Dragging(drag) = self.drag else {
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
