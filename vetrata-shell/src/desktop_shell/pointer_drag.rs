use std::mem;

use iced::{Subscription, Task, window::frames};
use niri_ipc::{Action, WorkspaceReferenceArg};
use tokio::time::sleep;
use vetrata_sources::niri;
use vetrata_state::WorkspacesPlacement;
use vetrata_widgets::{
    bar_slot_drag::{DragMessage, SlotClick},
    shell_message::Message,
    widgets::pointer_drag::{
        self, ActiveDrag, DragOutcome, DragTransition, DragView, DraggedBarSlot, PointerDrag,
    },
};

use crate::{desktop_shell::DesktopShell, message_subscriptions::niri_action};

pub type ShellDrag = PointerDrag<SlotClick>;

impl DesktopShell {
    #[must_use]
    pub fn drag_view(&self) -> DragView {
        let dragging_sidebar_workspace = self.shell_settings.layout.workspaces_placement
            == WorkspacesPlacement::Sidebar
            && matches!(
                self.pointer_drag,
                PointerDrag::Dragging(ActiveDrag {
                    item: DraggedBarSlot::Workspace(_),
                    ..
                })
            );
        let ghost_layer = if dragging_sidebar_workspace {
            &self.shell_settings.sidebar
        } else {
            &self.shell_settings.panels
        };
        DragView::new(&self.pointer_drag, &ghost_layer.frame_style(&self.config))
    }

    #[must_use]
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
                let DraggedBarSlot::Workspace(workspace) = hover.item else {
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

    fn follow_drag_outcome(&mut self, outcome: DragOutcome<SlotClick>) -> Task<Message> {
        match outcome {
            DragOutcome::Unchanged => Task::none(),
            DragOutcome::Started => self.dismiss_tooltip(),
            DragOutcome::Clicked(click) => click_slot(click),
            DragOutcome::Dropped(drag) => self.drop_dragged(drag),
            DragOutcome::HoveringForeign(hover) => match hover.item {
                DraggedBarSlot::Workspace(_) => {
                    Task::future(sleep(self.config.panel_bar.drag_spring_load))
                        .map(move |()| Message::PointerDrag(DragMessage::HoverSpringLoaded(hover)))
                }
                DraggedBarSlot::Window { .. } => Task::none(),
            },
        }
    }

    fn drop_dragged(&mut self, drag: ActiveDrag) -> Task<Message> {
        if let Some(hover) = drag.hovered()
            && let DraggedBarSlot::Workspace(workspace) = hover.item
            && let DraggedBarSlot::Window { window, .. } = drag.item
        {
            return self.move_window_to_workspace(window, workspace);
        }
        match drag.reordered_to() {
            Some(to) => self.reorder(drag, to),
            None => Task::none(),
        }
    }

    fn reorder(&mut self, drag: ActiveDrag, to: usize) -> Task<Message> {
        let Some(snapshot) = self.niri_snapshot.get() else {
            return Task::none();
        };
        match drag.item {
            DraggedBarSlot::Workspace(workspace) => {
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
            DraggedBarSlot::Window { window, workspace } => {
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
