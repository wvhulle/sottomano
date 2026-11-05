use iced::{
    Element, mouse,
    widget::{MouseArea, mouse_area},
};

use crate::pointer_drag::{DragEvent, DragView, SlotPress};

pub fn draggable_slot<
    'a,
    Message: Clone + 'a,
    SlotId: Copy + PartialEq,
    ListId,
    ClickPayload: Copy,
>(
    content: impl Into<Element<'a, Message>>,
    press: SlotPress<SlotId, ClickPayload>,
    reorders_on_hover: bool,
    drag: &DragView<SlotId>,
    on_drag: impl Fn(DragEvent<SlotId, ListId, ClickPayload>) -> Message,
) -> MouseArea<'a, Message> {
    let area = mouse_area(content)
        .interaction(if drag.is_active() {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Pointer
        })
        .on_press(on_drag(DragEvent::Pressed(press)));
    if drag.is_active() && reorders_on_hover {
        area.on_enter(on_drag(DragEvent::EnteredSlot {
            item: press.item,
            position: press.position,
        }))
    } else {
        area
    }
}
