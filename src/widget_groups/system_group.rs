//! odin's `System` group: cpu handle expands to temperature/memory/disk.

use crate::concrete_widgets::{MenuId, Widget};
use crate::widget_groups::Group;

pub fn group() -> Group {
    Group::Menu {
        id: MenuId::System,
        handle: Widget::Cpu,
        content: vec![Widget::Temperature, Widget::Memory, Widget::Disk],
    }
}
