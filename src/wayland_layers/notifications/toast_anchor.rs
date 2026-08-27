use iced::{Element, Rectangle};

use super::toast_lifecycle::NotificationMessage;
use crate::data::source::niri::{HEdge, VEdge};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToastAnchor {
    Edge { vertical: VEdge, horizontal: HEdge },
    Window { id: u64, bounds: Rectangle },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnchorGroup {
    Edge { vertical: VEdge, horizontal: HEdge },
    Window(u64),
}

impl ToastAnchor {
    pub fn is_window_bound(self) -> bool {
        matches!(self, Self::Window { .. })
    }

    pub fn group(self) -> AnchorGroup {
        match self {
            Self::Edge {
                vertical,
                horizontal,
            } => AnchorGroup::Edge {
                vertical,
                horizontal,
            },
            Self::Window { id, .. } => AnchorGroup::Window(id),
        }
    }
}

pub struct ToastPlacement {
    pub output: Option<String>,
    pub anchor: ToastAnchor,
}

pub struct ToastSlot<'a> {
    pub id: u32,
    pub anchor: ToastAnchor,
    pub card: Element<'a, NotificationMessage>,
    pub animating: bool,
}
