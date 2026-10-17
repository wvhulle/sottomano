use std::mem;

use iced::{Subscription, Task, window::frames};
use niri_ipc::{Action, WorkspaceReferenceArg};
use tokio::time::sleep;

use crate::{
    data::source::niri,
    desktop_shell::{DesktopShell, Message},
    message_subscriptions::niri_action,
    widgets::pointer_drag::{
        self, ActiveDrag, DragEvent, DragOutcome, DragTransition, ForeignHover, PointerDrag,
        SlotItem,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DraggedItem {
    Workspace(u64),
    Window { window: u64, workspace: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotList {
    Workspaces,
    Windows,
}

impl SlotItem for DraggedItem {
    type List = SlotList;

    fn list(self) -> SlotList {
        match self {
            DraggedItem::Workspace(_) => SlotList::Workspaces,
            DraggedItem::Window { .. } => SlotList::Windows,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SlotClick {
    Workspace { id: u64, is_active: bool },
    FocusWindow(u64),
    MaximizeColumn,
}

pub type ShellDrag = PointerDrag<DraggedItem, SlotClick>;
pub type DragView = pointer_drag::DragView<DraggedItem>;
pub type SlotPress = pointer_drag::SlotPress<DraggedItem, SlotClick>;
pub type SlotDragEvent = DragEvent<DraggedItem, SlotClick>;

#[derive(Debug, Clone, Copy)]
pub enum DragMessage {
    Pointer(SlotDragEvent),
    HoverSpringLoaded(ForeignHover<DraggedItem>),
}

impl DragView {
    pub fn window_source_workspace(&self) -> Option<u64> {
        match self.drag?.item {
            DraggedItem::Window { workspace, .. } => Some(workspace),
            DraggedItem::Workspace(_) => None,
        }
    }

    pub fn awaits_window_on(&self, workspace: u64) -> bool {
        self.window_source_workspace().is_some()
            && self.hovers(DraggedItem::Workspace(workspace)).is_some()
    }
}

impl DesktopShell {
    pub fn drag_view(&self) -> DragView {
        DragView::new(
            &self.pointer_drag,
            &self.shell_settings.frame_style(&self.config),
        )
    }

    pub fn is_dragging(&self) -> bool {
        self.pointer_drag.is_dragging()
    }

    pub fn pointer_drag_subscription(&self) -> Subscription<Message> {
        let pointer = pointer_drag::pointer_events(!self.pointer_drag.is_idle())
            .map(|event| Message::PointerDrag(DragMessage::Pointer(event)));
        let spring_charging = match self.drag_view().drag.and_then(|drag| drag.hovered()) {
            Some(_) => frames().map(|_| Message::Redraw),
            None => Subscription::none(),
        };
        Subscription::batch([pointer, spring_charging])
    }

    pub fn update_pointer_drag(&mut self, msg: DragMessage) -> Task<Message> {
        match msg {
            DragMessage::Pointer(event) => {
                let DragTransition { drag, outcome } = pointer_drag::advance(
                    mem::replace(&mut self.pointer_drag, PointerDrag::Idle),
                    event,
                    self.config.panel_bar.drag_start_distance,
                );
                self.pointer_drag = drag;
                self.follow_drag_outcome(outcome)
            }
            DragMessage::HoverSpringLoaded(hover) => {
                let DraggedItem::Workspace(workspace) = hover.item else {
                    return Task::none();
                };
                if self.drag_view().drag.and_then(|drag| drag.hovered()) != Some(hover) {
                    return Task::none();
                }
                niri_action(Action::FocusWorkspace {
                    reference: WorkspaceReferenceArg::Id(workspace),
                })
            }
        }
    }

    fn follow_drag_outcome(
        &mut self,
        outcome: DragOutcome<DraggedItem, SlotClick>,
    ) -> Task<Message> {
        match outcome {
            DragOutcome::Unchanged => Task::none(),
            DragOutcome::Started => self.dismiss_tooltip(),
            DragOutcome::Clicked(click) => click_slot(click),
            DragOutcome::Dropped(drag) => self.drop_dragged(drag),
            DragOutcome::HoveringForeign(hover) => match hover.item {
                DraggedItem::Workspace(_) => {
                    Task::future(sleep(self.config.panel_bar.drag_spring_load))
                        .map(move |()| Message::PointerDrag(DragMessage::HoverSpringLoaded(hover)))
                }
                DraggedItem::Window { .. } => Task::none(),
            },
        }
    }

    fn drop_dragged(&mut self, drag: ActiveDrag<DraggedItem>) -> Task<Message> {
        if let Some(hover) = drag.hovered()
            && let DraggedItem::Workspace(workspace) = hover.item
            && let DraggedItem::Window { window, .. } = drag.item
        {
            return self.move_window_to_workspace(window, workspace);
        }
        match drag.reordered_to() {
            Some(to) => self.reorder(drag, to),
            None => Task::none(),
        }
    }

    fn reorder(&mut self, drag: ActiveDrag<DraggedItem>, to: usize) -> Task<Message> {
        let Some(snapshot) = self.niri_snapshot.get() else {
            return Task::none();
        };
        match drag.item {
            DraggedItem::Workspace(workspace) => {
                let Some(index) = self
                    .surfaces
                    .get(&drag.surface)
                    .and_then(|surface| snapshot.workspaces_for_output(&surface.output).get(to))
                    .map(|target| usize::from(target.idx))
                else {
                    return Task::none();
                };
                self.reshape_snapshot(|snapshot| snapshot.with_workspace_moved(workspace, index));
                niri_action(Action::MoveWorkspaceToIndex {
                    index,
                    reference: Some(WorkspaceReferenceArg::Id(workspace)),
                })
            }
            DraggedItem::Window { window, workspace } => {
                let Some(index) = snapshot
                    .windows_on_workspace(workspace)
                    .get(to)
                    .and_then(|target| target.layout.column)
                else {
                    return Task::none();
                };
                self.reshape_snapshot(|snapshot| snapshot.with_column_moved(window, index));
                niri_action(Action::FocusWindow { id: window })
                    .chain(niri_action(Action::MoveColumnToIndex { index }))
            }
        }
    }

    pub fn restore_workspace_order(&self, output: &str) -> Task<Message> {
        let Some(configured) = &self.configured_workspaces else {
            return Task::none();
        };
        let Some(snapshot) = self.niri_snapshot.get() else {
            return Task::none();
        };
        configured
            .restoring_moves(snapshot.workspaces_for_output(output))
            .into_iter()
            .map(|restored| {
                niri_action(Action::MoveWorkspaceToIndex {
                    index: restored.index,
                    reference: Some(WorkspaceReferenceArg::Name(restored.name)),
                })
            })
            .fold(Task::none(), Task::chain)
    }

    fn move_window_to_workspace(&mut self, window: u64, workspace: u64) -> Task<Message> {
        if self
            .niri
            .windows
            .windows
            .get(&window)
            .is_none_or(|found| found.workspace_id == Some(workspace))
        {
            return Task::none();
        }
        self.reshape_snapshot(|snapshot| {
            snapshot.with_window_moved_to_workspace(window, workspace)
        });
        niri_action(Action::MoveWindowToWorkspace {
            window_id: Some(window),
            reference: WorkspaceReferenceArg::Id(workspace),
            focus: false,
        })
    }

    fn reshape_snapshot(&mut self, reshape: impl FnOnce(niri::Snapshot) -> niri::Snapshot) {
        if let Some(snapshot) = self.niri_snapshot.get().cloned() {
            self.niri_snapshot.load(reshape(snapshot));
        }
    }
}

fn click_slot(click: SlotClick) -> Task<Message> {
    match click {
        SlotClick::Workspace {
            is_active: true, ..
        } => niri_action(Action::ToggleOverview {}),
        SlotClick::Workspace { id, .. } => niri_action(Action::FocusWorkspace {
            reference: WorkspaceReferenceArg::Id(id),
        }),
        SlotClick::FocusWindow(window) => niri_action(Action::FocusWindow { id: window }),
        SlotClick::MaximizeColumn => niri_action(Action::MaximizeColumn {}),
    }
}
