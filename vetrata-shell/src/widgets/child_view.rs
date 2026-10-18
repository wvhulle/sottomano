use iced::{
    Element,
    advanced::{Layout, widget::Tree},
};

pub struct ChildView<'t, 'a, Message> {
    pub widget: &'t Element<'a, Message>,
    pub state: &'t Tree,
    pub layout: Layout<'t>,
}

pub struct ChildViewMut<'t, 'a, Message> {
    pub widget: &'t mut Element<'a, Message>,
    pub state: &'t mut Tree,
    pub layout: Layout<'t>,
}
