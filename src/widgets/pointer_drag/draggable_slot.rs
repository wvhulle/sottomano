use iced::{
    Element, mouse,
    widget::{MouseArea, mouse_area},
};

use crate::widgets::pointer_drag::{DragEvent, DragView, SlotItem, SlotPress};

pub fn draggable_slot<'a, Message: Clone + 'a, Item: SlotItem, Click: Copy>(
    content: impl Into<Element<'a, Message>>,
    press: SlotPress<Item, Click>,
    reorders_on_hover: bool,
    drag: &DragView<Item>,
    on_drag: impl Fn(DragEvent<Item, Click>) -> Message,
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

pub fn draggable_slot_list<'a, Message: Clone + 'a, Item: SlotItem, Click>(
    slots: impl Into<Element<'a, Message>>,
    list: Item::List,
    drag: &DragView<Item>,
    on_drag: impl Fn(DragEvent<Item, Click>) -> Message,
) -> Element<'a, Message> {
    let area = mouse_area(slots);
    if drag.is_active() {
        area.on_exit(on_drag(DragEvent::LeftList(list))).into()
    } else {
        area.into()
    }
}
