use corredo::drag_reorder::{GhostPlacement, ListDeparture, drag_ghost};
use iced::{
    Element, Length, Vector,
    widget::{column, container, stack},
    window::Id,
};
use sottomano_toml_config::screen_edges::Position;
use sottomano_widgets::{
    hover_cards::{AnchorEvent, HoverCardMessage, PopupMessage, WidgetAnchor},
    widget_groups::{
        widget_group_interaction::WidgetGroupInteraction, window_group::WindowGroupInteraction,
        workspaces_group,
    },
    widgets::pointer_drag::DraggedPanelSlot,
};

use crate::{
    desktop_shell::DesktopShell,
    panels::{bottom_panel::BOTTOM_PANEL, top_panel::TOP_PANEL},
    shell_message::ShellInteraction,
};

fn anchor_hover_cards_on_surface(
    surface: Id,
    position: Position,
    origin_in_overlay: Vector,
    content: Element<'_, ShellInteraction>,
) -> Element<'_, ShellInteraction> {
    content.map(move |message| match message {
        ShellInteraction::Popup(PopupMessage::HoverCard(HoverCardMessage::Anchor(event))) => {
            ShellInteraction::from(HoverCardMessage::OnSurface {
                surface,
                position,
                event: Box::new(match event {
                    AnchorEvent::LaidOut(widget_anchor) => AnchorEvent::LaidOut(WidgetAnchor {
                        anchor: widget_anchor.anchor + origin_in_overlay,
                        ..widget_anchor
                    }),
                    pointer_event => pointer_event,
                }),
            })
        }
        unanchored => unanchored,
    })
}

impl DesktopShell {
    pub(crate) fn monitor_layout<'a>(
        &'a self,
        window: Id,
        output: &'a str,
    ) -> Element<'a, ShellInteraction> {
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

    fn drag_ghost_layer<'a>(
        &'a self,
        window: Id,
        output: &'a str,
    ) -> Option<Element<'a, ShellInteraction>> {
        let slot_context = self.panel_slot_context(Position::Bottom);
        let view = slot_context.drag;
        let drag = view.drag.filter(|drag| drag.surface == window)?;
        let snapshot = &self.window_snapshot;
        let card = match drag.item {
            DraggedPanelSlot::Workspace(workspace) => {
                workspaces_group::drag_ghost(snapshot, output, workspace, slot_context)?
                    .map(ShellInteraction::from)
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
                .map(|message| {
                    ShellInteraction::Group(WidgetGroupInteraction::WindowGroup(
                        WindowGroupInteraction::WindowList(message),
                    ))
                }),
        };
        let placement = match drag.item {
            DraggedPanelSlot::Window { .. }
                if view.list_departure() == ListDeparture::LeftOwnList =>
            {
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
