use iced::{
    Element, Length, Vector,
    widget::{column, container, stack},
    window::Id,
};

use crate::{
    desktop_shell::{
        DesktopShell, Message, hover_tooltips::on_surface, monitor_surfaces::Position,
        pointer_drag::DraggedItem,
    },
    widget_groups::{window_group::WindowGroupMessage, workspaces_group},
    widgets::pointer_drag::drag_ghost::{GhostPlacement, drag_ghost},
};

impl DesktopShell {
    pub(super) fn monitor_layout<'a>(
        &'a self,
        window: Id,
        output: &'a str,
    ) -> Element<'a, Message> {
        let float = self.shell_settings.layout.float_offset_px;
        let origin = Vector::new(-float, -float);
        let on_edge = |position, content| on_surface(window, position, origin, content);
        let workspace_sidebar = container(on_edge(
            Position::Left,
            self.workspace_sidebar(window, output),
        ))
        .align_left(Length::Fill)
        .center_y(Length::Fill);
        let notification_toasts = Element::from(self.notifications.anchored_toasts(
            output,
            &self.config,
            &self.theme,
            &self.shell_settings,
            self.bar_clearance(output),
        ))
        .map(Message::Notification);
        stack![
            notification_toasts,
            column![
                on_edge(Position::Top, self.top_bar()),
                workspace_sidebar,
                on_edge(Position::Bottom, self.bottom_bar(output)),
            ]
            .padding(float)
        ]
        .extend(self.drag_ghost_layer(window, output))
        .into()
    }

    fn drag_ghost_layer<'a>(&'a self, window: Id, output: &'a str) -> Option<Element<'a, Message>> {
        let view = self.drag_view();
        let drag = view.drag.filter(|drag| drag.surface == window)?;
        let snapshot = self.niri_snapshot.get()?;
        let card = match drag.item {
            DraggedItem::Workspace(workspace) => {
                workspaces_group::drag_ghost(snapshot, output, workspace, &self.config, &view)?
                    .map(Message::from)
            }
            DraggedItem::Window { window, .. } => self
                .window_group
                .window_list
                .drag_ghost(
                    snapshot,
                    self.window_assets(),
                    self.audio_group.window_audio(),
                    window,
                    &self.config,
                    &view,
                )?
                .map(|message| Message::WindowGroup(WindowGroupMessage::WindowList(message))),
        };
        let placement = match drag.item {
            DraggedItem::Window { .. } if view.has_left_its_list() => {
                GhostPlacement::BesideCursor {
                    gap: self.config.theme.font.icon_size_px,
                }
            }
            DraggedItem::Window { .. } | DraggedItem::Workspace(_) => GhostPlacement::UnderCursor,
        };
        Some(drag_ghost(
            card,
            &drag,
            placement,
            self.config.panel_bar.drag_ghost_scale,
        ))
    }
}
