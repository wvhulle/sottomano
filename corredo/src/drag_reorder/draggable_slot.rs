use iced::{
    Element, mouse,
    widget::{MouseArea, mouse_area},
};

use crate::drag_reorder::{DragEvent, DragView, SlotPress};

pub fn draggable_slot<
    'a,
    Message: Clone + 'a,
    SlotId: Copy + PartialEq,
    ListId,
    ClickPayload: Copy,
>(
    content: impl Into<Element<'a, Message>>,
    press: SlotPress<SlotId, ClickPayload>,
    hover_reorder: HoverReorder,
    drag: &DragView<SlotId>,
    on_drag: impl Fn(DragEvent<SlotId, ListId, ClickPayload>) -> Message,
) -> MouseArea<'a, Message> {
    let area = mouse_area(content)
        .interaction(match drag.drag {
            Some(_) => mouse::Interaction::Grabbing,
            None => mouse::Interaction::Pointer,
        })
        .on_press(on_drag(DragEvent::Pressed(press)));
    if drag.drag.is_some() && hover_reorder == HoverReorder::ReordersOnHover {
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
