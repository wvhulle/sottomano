//! The centered clock: a single-widget static group.

use crate::concrete_widgets::Widget;
use crate::widget_groups::Group;

pub fn group() -> Group {
    Group::Static(vec![Widget::Time])
}
