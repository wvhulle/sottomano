use std::mem;

use iced::{Subscription, Task, window::frames};
use niri_ipc::{Action, WorkspaceReferenceArg};
use sottomano_desktop_sources::desktop_windows_apps::workspace_window_snapshot;
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    background_work::niri_action,
    desktop_panels::panel_slot_drag::{DragMessage, PanelSlotContext, SlotClick},
    widget_groups::{
        sidebar_settings_group::WorkspacesPlacement, workspaces_group::shown_workspaces,
    },
    widgets::{
        pointer_drag,
        pointer_drag::{
            ActiveDrag, DragOutcome, DragTransition, DragView, DraggedPanelSlot, PointerDrag,
        },
    },
};
use tokio::time::sleep;

use crate::{desktop_shell::DesktopShell, shell_message::Message};

pub type ShellDrag = PointerDrag<SlotClick>;

impl DesktopShell {
    #[must_use]
    pub fn drag_view(&self) -> DragView {
        let dragging_sidebar_workspace = self
            .widget_groups
            .sidebar_settings_group
            .workspaces_placement()
            == WorkspacesPlacement::Sidebar
            && matches!(
                self.pointer_drag,
                PointerDrag::Dragging(ActiveDrag {
                    item: DraggedPanelSlot::Workspace(_),
                    ..
                })
            );
        let ghost_layer = if dragging_sidebar_workspace {
            self.widget_groups
                .sidebar_settings_group
                .sidebar_appearance()
        } else {
            self.widget_groups.panel_settings_group.panel_appearance()
        };
        DragView::new(&self.pointer_drag, ghost_layer.frame_style())
    }

    #[must_use]
    pub fn panel_slot_context(&self, edge: Position) -> PanelSlotContext<'_> {
        let panel_appearance = self.widget_groups.panel_settings_group.panel_appearance();
        PanelSlotContext {
            config: &self.config,
            edge,
            drag: self.drag_view(),
            clock: self.clock,
            panel_corner_radius_px: panel_appearance.corner_radius_px,
            panel_frame_style: panel_appearance.frame_style(),
            window_row_layout: self.widget_groups.panel_settings_group.window_row_layout(),
        }
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
                    self.clock.instant,
                );
                self.pointer_drag = drag;
                self.follow_drag_outcome(outcome)
            }
            DragMessage::HoverSpringLoaded(hover) => {
                let DraggedPanelSlot::Workspace(workspace) = hover.item else {
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
            DragOutcome::Started => self.dismiss_hover_card(),
            DragOutcome::Clicked(click) => click_slot(click),
            DragOutcome::Dropped(drag) => self.drop_dragged(drag),
            DragOutcome::HoveringForeign(hover) => match hover.item {
                DraggedPanelSlot::Workspace(_) => {
                    let (task, handle) = Task::future(sleep(
                        self.config.desktop_panels.slot_drag.spring_load_delay,
                    ))
                    .map(move |()| Message::PointerDrag(DragMessage::HoverSpringLoaded(hover)))
                    .abortable();
                    self.spring_load = Some(handle.abort_on_drop());
                    task
                }
                DraggedPanelSlot::Window { .. } => {
                    self.spring_load = None;
                    Task::none()
                }
            },
        }
    }

    fn drop_dragged(&mut self, drag: ActiveDrag) -> Task<Message> {
        if let Some(hover) = drag.hovered()
            && let DraggedPanelSlot::Workspace(workspace) = hover.item
            && let DraggedPanelSlot::Window { window, .. } = drag.item
        {
            return self.move_window_to_workspace(window, workspace);
        }
        match drag.reordered_to() {
            Some(to) => self.reorder(drag, to),
            None => Task::none(),
        }
    }

    fn reorder(&mut self, drag: ActiveDrag, to: usize) -> Task<Message> {
        let snapshot = &self.window_snapshot;
        match drag.item {
            DraggedPanelSlot::Workspace(workspace) => {
                let Some(index) = self
                    .surfaces
                    .get(&drag.surface)
                    .and_then(|surface| {
                        shown_workspaces(
                            snapshot,
                            &surface.output,
                            self.widget_groups
                                .sidebar_settings_group
                                .empty_workspace_folding(),
                        )
                        .get(to)
                        .copied()
                    })
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
            DraggedPanelSlot::Window { window, workspace } => {
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
        let snapshot = &self.window_snapshot;
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
            .compositor
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

    fn reshape_snapshot(
        &mut self,
        reshape: impl FnOnce(workspace_window_snapshot::Snapshot) -> workspace_window_snapshot::Snapshot,
    ) {
        self.window_snapshot = reshape(mem::take(&mut self.window_snapshot));
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
