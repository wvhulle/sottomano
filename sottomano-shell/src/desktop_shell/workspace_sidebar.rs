use iced::{
    Element,
    widget::{Column, container},
    window::Id,
};
use sottomano_config::screen_edges::Position;
use sottomano_state::WorkspacesPlacement;
use sottomano_widgets::{
    shell_message::Message,
    widget_groups::workspaces_group,
    widgets::{
        panel_frame::{LayerFrameStyle, OutlinedSides, panel_frame},
        region_reporter::{RegionRoles, region_reporter},
    },
};

use crate::desktop_shell::DesktopShell;

impl DesktopShell {
    pub(super) fn workspace_sidebar<'a>(
        &'a self,
        window: Id,
        output: &str,
    ) -> Element<'a, Message> {
        let config = &self.config;
        let sidebar = &self.shell_settings.sidebar;
        let frame_style = LayerFrameStyle::new(sidebar, config).container_style();
        let outlined_sides =
            OutlinedSides::inward_of(Position::Left).all_when_floating(&self.shell_settings.layout);
        let framed_workspaces = (self.shell_settings.layout.workspaces_placement
            == WorkspacesPlacement::Sidebar)
            .then(|| self.niri_snapshot.get())
            .flatten()
            .and_then(|snapshot| {
                workspaces_group::vertical_view(
                    snapshot,
                    output,
                    config,
                    &self.drag_view(),
                    self.configured_workspaces.as_ref(),
                )
            })
            .map(|workspaces| {
                panel_frame(
                    container(workspaces.map(Message::from))
                        .padding([sidebar.corner_radius_px, 0.0]),
                    &frame_style,
                )
                .outlined_sides(outlined_sides)
                .into()
            });
        region_reporter(
            Column::with_children(framed_workspaces),
            RegionRoles {
                takes_input: true,
                blurred: sidebar.blurred,
                corner_radius: sidebar.corner_radius_px,
            },
        )
        .on_width_change(move |width| Message::SidebarWidthChanged { window, width })
        .into()
    }
}
