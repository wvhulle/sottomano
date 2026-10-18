use crate::widgets::pointer_drag::{self, DragEvent, DragView, DraggedBarSlot, ForeignHover};

#[derive(Debug, Clone, Copy)]
pub enum SlotClick {
    Workspace { id: u64, is_active: bool },
    FocusWindow(u64),
    MaximizeColumn,
}

pub type SlotPress = pointer_drag::SlotPress<SlotClick>;
pub type SlotDragEvent = DragEvent<SlotClick>;

#[derive(Debug, Clone, Copy)]
pub enum DragMessage {
    Pointer(SlotDragEvent),
    HoverSpringLoaded(ForeignHover),
}

impl DragView {
    #[must_use]
    pub fn window_source_workspace(&self) -> Option<u64> {
        match self.drag?.item {
            DraggedBarSlot::Window { workspace, .. } => Some(workspace),
            DraggedBarSlot::Workspace(_) => None,
        }
    }

    #[must_use]
    pub fn awaits_window_on(&self, workspace: u64) -> bool {
        self.window_source_workspace().is_some()
            && self.hovers(DraggedBarSlot::Workspace(workspace)).is_some()
    }
}
