use sottomano_config::{Config, screen_edges::Position};

use crate::{
    clock::Clock,
    widgets::pointer_drag::{self, DragEvent, DragView, DraggedPanelSlot, ForeignHover},
};

#[derive(Debug, Clone, Copy)]
pub enum SlotClick {
    Workspace { id: u64, is_active: bool },
    FocusWindow(u64),
    MaximizeColumn,
}

pub type SlotPress = pointer_drag::SlotPress<SlotClick>;
pub type SlotDragEvent = DragEvent<SlotClick>;

#[derive(Clone, Copy)]
pub struct PanelSlotContext<'a> {
    pub config: &'a Config,
    pub edge: Position,
    pub drag: DragView,
    pub clock: Clock,
}

#[derive(Debug, Clone, Copy)]
pub enum DragMessage {
    Pointer(SlotDragEvent),
    HoverSpringLoaded(ForeignHover),
}

impl DragView {
    #[must_use]
    pub fn window_source_workspace(&self) -> Option<u64> {
        match self.drag?.item {
            DraggedPanelSlot::Window { workspace, .. } => Some(workspace),
            DraggedPanelSlot::Workspace(_) => None,
        }
    }

    #[must_use]
    pub fn awaits_window_on(&self, workspace: u64) -> bool {
        self.window_source_workspace().is_some()
            && self
                .hovers(DraggedPanelSlot::Workspace(workspace))
                .is_some()
    }
}

#[cfg(test)]
pub(crate) mod pager_drag_simulation;
