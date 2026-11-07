use sottomano_toml_config::{Config, screen_edges::Position};

use crate::{
    clock::Clock,
    desktop_panels::panel_frame::FrameStyle,
    widget_groups::window_group::WindowRowLayout,
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
    pub panel_corner_radius_px: f32,
    pub panel_frame_style: FrameStyle,
    pub window_row_layout: WindowRowLayout,
}

#[derive(Debug, Clone, Copy)]
pub enum DragMessage {
    Pointer(SlotDragEvent),
    HoverSpringLoaded(ForeignHover),
}

#[must_use]
pub fn window_source_workspace(drag: &DragView) -> Option<u64> {
    match drag.drag?.item {
        DraggedPanelSlot::Window { workspace, .. } => Some(workspace),
        DraggedPanelSlot::Workspace(_) => None,
    }
}

#[must_use]
pub fn window_arrival_on(drag: &DragView, workspace: u64) -> WindowArrival {
    if window_source_workspace(drag).is_some()
        && drag
            .hovers(DraggedPanelSlot::Workspace(workspace))
            .is_some()
    {
        WindowArrival::Awaited
    } else {
        WindowArrival::NotAwaited
    }
}

#[cfg(test)]
pub(crate) mod pager_drag_simulation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowArrival {
    Awaited,
    NotAwaited,
}
