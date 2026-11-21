use iced::{
    Element,
    advanced::{Layout, widget::Tree},
};

pub struct ChildView<'tree, 'view, Message> {
    pub widget: &'tree Element<'view, Message>,
    pub state: &'tree Tree,
    pub layout: Layout<'tree>,
}

pub struct ChildViewMut<'tree, 'view, Message> {
    pub widget: &'tree mut Element<'view, Message>,
    pub state: &'tree mut Tree,
    pub layout: Layout<'tree>,
}
