use iced::{
    Element, mouse,
    widget::{MouseArea, mouse_area},
};

use crate::drag_reorder::{DragActivity, DragEvent, DragView, SlotPress};

pub fn draggable_slot<
    'view,
    Message: Clone + 'view,
    SlotId: Copy + PartialEq,
    ListId,
    ClickPayload: Copy,
>(
    content: impl Into<Element<'view, Message>>,
    press: SlotPress<SlotId, ClickPayload>,
    hover_reorder: HoverReorder,
    drag: &DragView<SlotId>,
    on_drag: impl Fn(DragEvent<SlotId, ListId, ClickPayload>) -> Message,
) -> MouseArea<'view, Message> {
    let area = mouse_area(content)
        .interaction(match drag.drag {
            DragActivity::Dragging(_) => mouse::Interaction::Grabbing,
            DragActivity::Idle => mouse::Interaction::Pointer,
        })
        .on_press(on_drag(DragEvent::Pressed(press)));
    if matches!(drag.drag, DragActivity::Dragging(_))
        && hover_reorder == HoverReorder::ReordersOnHover
    {
        area.on_enter(on_drag(DragEvent::EnteredSlot {
            item: press.item,
            position: press.position,
        }))
    } else {
        area
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverReorder {
    ReordersOnHover,
    StaysOnHover,
}
