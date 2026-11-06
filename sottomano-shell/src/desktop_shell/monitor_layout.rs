use iced::{
    Element, Length, Vector,
    widget::{column, container, stack},
    window::Id,
};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    hover_cards::{HoverCardMessage, WidgetAnchor},
    widget_groups::{window_group::WindowGroupMessage, workspaces_group},
    widgets::pointer_drag::{
        DraggedPanelSlot,
        drag_ghost::{GhostPlacement, drag_ghost},
    },
};

use crate::{
    desktop_shell::{
        DesktopShell,
        desktop_panels::panel_bar_composition::{BOTTOM_PANEL, TOP_PANEL},
    },
    shell_message::Message,
};

fn anchor_hover_cards_on_surface(
    surface: Id,
    position: Position,
    origin_in_overlay: Vector,
    content: Element<'_, Message>,
) -> Element<'_, Message> {
    content.map(move |message| match message {
        Message::HoverCard(HoverCardMessage::AnchorLaidOut(widget_anchor)) => {
            Message::HoverCard(HoverCardMessage::LaidOut {
                surface,
                widget_anchor: WidgetAnchor {
                    anchor: widget_anchor.anchor + origin_in_overlay,
                    ..widget_anchor
                },
            })
        }
        Message::HoverCard(HoverCardMessage::EnterAnchor {
            target,
            announcement,
        }) => Message::HoverCard(HoverCardMessage::Enter {
            surface,
            position,
            target,
            announcement,
        }),
        Message::HoverCard(HoverCardMessage::MenuHandlePointedAnchor {
            target,
            pointed_at,
            announcement,
        }) => Message::HoverCard(HoverCardMessage::MenuHandlePointed {
            surface,
            position,
            target,
            pointed_at,
            announcement,
        }),
        Message::HoverCard(HoverCardMessage::MenuHandlePressedAnchor {
            target,
            announcement,
        }) => Message::HoverCard(HoverCardMessage::MenuHandlePressed {
            surface,
            position,
            target,
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
        let float = self.widget_groups.panel_settings_group.float_offset_px();
        let origin = Vector::new(-float, -float);
        let on_edge =
            |position, content| anchor_hover_cards_on_surface(window, position, origin, content);
        let workspace_sidebar = container(on_edge(
            Position::Left,
            self.workspace_sidebar(window, output),
        ))
        .align_left(Length::Fill)
        .center_y(Length::Fill);
        stack![
            column![
                on_edge(Position::Top, self.panel_bar_view(&TOP_PANEL, output)),
                workspace_sidebar,
                on_edge(Position::Bottom, self.panel_bar_view(&BOTTOM_PANEL, output)),
            ]
            .padding(float),
            self.hover_card_layer(output),
        ]
        .extend(self.drag_ghost_layer(window, output))
        .into()
    }

    fn drag_ghost_layer<'a>(&'a self, window: Id, output: &'a str) -> Option<Element<'a, Message>> {
        let slot_context = self.panel_slot_context(Position::Bottom);
        let view = slot_context.drag;
        let drag = view.drag.filter(|drag| drag.surface == window)?;
        let snapshot = &self.window_snapshot;
        let card = match drag.item {
            DraggedPanelSlot::Workspace(workspace) => {
                workspaces_group::drag_ghost(snapshot, output, workspace, slot_context)?
                    .map(Message::from)
            }
            DraggedPanelSlot::Window { window, .. } => self
                .widget_groups
                .window_group
                .window_list
                .drag_ghost(
                    snapshot,
                    self.window_assets(),
                    self.widget_groups.audio_group.window_audio(),
                    window,
                    slot_context,
                )?
                .map(|message| Message::WindowGroup(WindowGroupMessage::WindowList(message))),
        };
        let placement = match drag.item {
            DraggedPanelSlot::Window { .. } if view.has_left_its_list() => {
                GhostPlacement::BesideCursor {
                    gap: self.config.theme.font.icon_size_px,
                }
            }
            DraggedPanelSlot::Window { .. } | DraggedPanelSlot::Workspace(_) => {
                GhostPlacement::UnderCursor
            }
        };
        Some(drag_ghost(
            card,
            &drag,
            placement,
            self.config.desktop_panels.slot_drag.ghost_scale,
        ))
    }
}
