use iced::{
    Element, mouse,
    widget::{MouseArea, mouse_area},
};

use crate::pointer_drag::{DragEvent, DragView, SlotPress};

pub fn draggable_slot<'a, Message: Clone + 'a, Item: Copy + PartialEq, List, Click: Copy>(
    content: impl Into<Element<'a, Message>>,
    press: SlotPress<Item, Click>,
    reorders_on_hover: bool,
    drag: &DragView<Item>,
    on_drag: impl Fn(DragEvent<Item, List, Click>) -> Message,
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
