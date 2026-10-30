use iced::{
    Element, Length, Vector,
    widget::{column, container, stack},
    window::Id,
};
use sottomano_config::screen_edges::Position;
use sottomano_widgets::{
    hover_tooltips::TooltipMessage,
    widget_groups::{window_group::WindowGroupMessage, workspaces_group},
    widgets::pointer_drag::{
        DraggedBarSlot,
        drag_ghost::{GhostPlacement, drag_ghost},
    },
};

use crate::{desktop_shell::DesktopShell, shell_message::Message};

fn anchor_tooltips_on_surface(
    surface: Id,
    position: Position,
    origin_in_overlay: Vector,
    content: Element<'_, Message>,
) -> Element<'_, Message> {
    content.map(move |message| match message {
        Message::Tooltip(TooltipMessage::EnterAnchor {
            target,
            anchor,
            announcement,
        }) => Message::Tooltip(TooltipMessage::Enter {
            surface,
            position,
            target,
            anchor: anchor + origin_in_overlay,
            announcement,
        }),
        unanchored => unanchored,
    })
}

impl DesktopShell {
    pub(super) fn monitor_layout<'a>(
        &'a self,
        window: Id,
        output: &'a str,
    ) -> Element<'a, Message> {
        let float = self.panel_settings_group.float_offset_px();
        let origin = Vector::new(-float, -float);
        let on_edge =
            |position, content| anchor_tooltips_on_surface(window, position, origin, content);
        let workspace_sidebar = container(on_edge(
            Position::Left,
            self.workspace_sidebar(window, output),
        ))
        .align_left(Length::Fill)
        .center_y(Length::Fill);
        stack![
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
        let snapshot = &self.niri_snapshot;
        let card = match drag.item {
            DraggedBarSlot::Workspace(workspace) => {
                workspaces_group::drag_ghost(snapshot, output, workspace, &self.config, &view)?
                    .map(Message::from)
            }
            DraggedBarSlot::Window { window, .. } => self
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
            DraggedBarSlot::Window { .. } if view.has_left_its_list() => {
                GhostPlacement::BesideCursor {
                    gap: self.config.theme.font.icon_size_px,
                }
            }
            DraggedBarSlot::Window { .. } | DraggedBarSlot::Workspace(_) => {
                GhostPlacement::UnderCursor
            }
        };
        Some(drag_ghost(
            card,
            &drag,
            placement,
            self.config.panel_bar.drag_ghost_scale,
        ))
    }
}
